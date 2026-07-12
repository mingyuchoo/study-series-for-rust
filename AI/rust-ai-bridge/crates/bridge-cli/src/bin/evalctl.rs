//! 품질 평가 CLI — 턴·레이팅·골든셋·자동 채점.

use ai_bridge::{SystemsOptions,
                app,
                approval::SqliteApprovalStore,
                audit::SqliteLogger,
                auth,
                eval::{self,
                       SqliteEvalStore,
                       Store as _,
                       golden::{Runner,
                                Suite},
                       judge::{Auto,
                               LlmJudge}},
                gateway::{Deps as GwDeps,
                          Gateway,
                          StoreApprover},
                inventory::Inventory,
                policy};
use anyhow::{Result,
             bail};
use clap::{Parser,
           Subcommand};
use std::{path::PathBuf,
          sync::Arc};

#[derive(Parser)]
#[command(name = "evalctl", version, about = "품질 평가")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    #[arg(long, global = true, default_value = "eval.db")]
    db: PathBuf,
}

#[derive(Subcommand)]
enum Cmd {
    /// 턴 목록.
    Turns {
        #[arg(long, default_value_t = 20)]
        limit: i64,
        #[arg(long)]
        actor: Option<String>,
        #[arg(long)]
        unrated: bool,
    },
    /// 턴 상세.
    Show { turn_id: String },
    /// 관리자 레이블 (append-only, source=human_reviewer).
    Rate {
        turn_id: String,
        #[arg(long, value_parser = ["up", "down"])]
        thumbs: String,
        #[arg(long)]
        by: String,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        label: Vec<String>,
    },
    /// 골든셋 목록.
    Suites {
        #[arg(long, default_value = "eval/suites")]
        dir: PathBuf,
    },
    /// 골든셋 배치 실행.
    Run {
        #[arg(long)]
        suite: String,
        #[arg(long, default_value = "eval/suites")]
        dir: PathBuf,
        /// 통과율이 이보다 낮으면 종료 코드 1 (CI 게이트).
        #[arg(long, default_value_t = 1.0)]
        fail_under: f64,
        #[arg(long, default_value = "config/policies.yaml")]
        policy: PathBuf,
        #[arg(long, default_value = "config/principal.yaml")]
        principal: PathBuf,
        #[arg(long, default_value = "config/systems.yaml")]
        systems: PathBuf,
        #[arg(long)]
        record_turns: bool,
        #[arg(long)]
        no_store: bool,
    },
    /// 골든셋 실행 이력.
    Runs {
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    /// 한 턴 자동 채점.
    Judge {
        turn_id: String,
        #[arg(long)]
        llm: bool,
    },
    /// 여러 턴 일괄 자동 채점.
    JudgeAll {
        #[arg(long, default_value_t = 50)]
        limit: i64,
        #[arg(long, default_value = "168h")]
        since: humantime::Duration,
        #[arg(long)]
        llm: bool,
    },
}

/// 골든셋 러너를 조립합니다.
async fn build_runner(
    policy_path: &std::path::Path,
    principal_path: &std::path::Path,
    systems_path: &std::path::Path,
    store: Option<Arc<dyn eval::Store>>,
    record_turns: bool,
) -> Result<Runner> {
    let inv = Arc::new(Inventory::load(systems_path)?);
    let pol = Arc::new(policy::Engine::load(policy_path)?);

    let opts = SystemsOptions {
        inventory: Some(inv.clone()),
        ..Default::default()
    };

    let make_gw =
        |allow_high_risk: bool, reg: Arc<ai_bridge::registry::Registry>, audit: Arc<SqliteLogger>, approvals: Arc<SqliteApprovalStore>| -> Arc<Gateway> {
            Arc::new(Gateway::new(GwDeps {
                registry: reg,
                policy: pol.clone(),
                audit,
                approver: Some(Arc::new(StoreApprover::new(approvals))),
                inventory: Some(inv.clone()),
                masker: None,
                limiter: None,
                breaker: None,
                budget: None,
                telemetry: None,
                injection: None,
                allow_high_risk,
            }))
        };

    // 골든셋은 임시 인메모리 저장소를 씁니다 — 감사 데이터를 실제로 남기지
    // 않습니다.
    let audit = Arc::new(SqliteLogger::open_in_memory().await?);
    let approvals = Arc::new(SqliteApprovalStore::open_in_memory().await?);

    let adapters = legacies::build_adapters(&opts, None).await?;
    let reg = Arc::new(app::build_registry(&adapters)?);
    app::validate_inventory(&reg, &inv)?;
    policy::validate_references(&pol.snapshot(), &reg, &inv)?;

    Ok(Runner {
        gateway: make_gw(false, reg.clone(), audit.clone(), approvals.clone()),
        gateway_high: make_gw(true, reg, audit, approvals),
        principals: auth::load_directory(principal_path)?,
        store,
        record_turns,
        git_sha: std::env::var("GIT_SHA").unwrap_or_default(),
        model: "scripted-gateway".into(),
    })
}

fn open_judge_llm() -> Result<Arc<dyn ai_bridge::llm::Provider>> { Ok(ai_bridge::llm::from_env()?.into()) }

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        | Cmd::Turns {
            limit,
            actor,
            unrated,
        } => {
            let store = SqliteEvalStore::open(&cli.db).await?;
            let f = eval::TurnFilter {
                actor: actor.unwrap_or_default(),
                unrated_only: unrated,
                limit,
                ..Default::default()
            };
            let turns = store.query_turns(&f).await?;
            if turns.is_empty() {
                println!("(턴 없음)");
                return Ok(());
            }
            println!("{:<20} {:<20} {:<16} {:<10} {:<10} PROMPT", "TURN_ID", "TS", "ACTOR", "CHANNEL", "OUTCOME");
            for t in turns {
                let prompt = t.prompt.split_whitespace().collect::<Vec<_>>().join(" ");
                let prompt: String = prompt.chars().take(40).collect();
                println!(
                    "{:<20} {:<20} {:<16} {:<10} {:<10} {}",
                    t.turn_id,
                    t.timestamp.map(|x| x.to_rfc3339()).unwrap_or_default(),
                    t.actor,
                    t.channel,
                    t.outcome,
                    prompt
                );
            }
        },

        | Cmd::Show {
            turn_id,
        } => {
            let store = SqliteEvalStore::open(&cli.db).await?;
            let t = store.get_turn(&turn_id).await?;
            println!("turn_id = {}", t.turn_id);
            println!("actor   = {}  agent = {}", t.actor, t.agent_id);
            println!("channel = {}  outcome = {}", t.channel, t.outcome);
            println!("hash    = {}", t.content_hash);
            println!("\n--- prompt ---\n{}", t.prompt);
            println!("\n--- reply ---\n{}", t.reply);
            if !t.tool_trail.is_empty() {
                println!("\n--- tool trail ---");
                for (i, s) in t.tool_trail.iter().enumerate() {
                    println!("  {}. {} decision={} err={}", i, s.name, s.decision, s.error_code);
                }
            }
            let ratings = store
                .query_ratings(&eval::RatingFilter {
                    turn_id: turn_id.clone(),
                    ..Default::default()
                })
                .await?;
            println!("\n--- ratings ---");
            if ratings.is_empty() {
                println!("  (없음)");
            }
            for r in ratings {
                println!(
                    "  #{} {} source={} rater={} score={:.2} labels={:?}",
                    r.id,
                    r.timestamp.map(|x| x.to_rfc3339()).unwrap_or_default(),
                    r.source,
                    r.rater_id,
                    r.score,
                    r.labels
                );
            }
        },

        | Cmd::Rate {
            turn_id,
            thumbs,
            by,
            note,
            label,
        } => {
            if by.is_empty() {
                bail!("--by 평가자 ID가 필요합니다");
            }
            for l in &label {
                if !eval::is_known_label(l) {
                    bail!("알 수 없는 라벨 {l:?}. 가능: {}", eval::KNOWN_LABELS.join(", "));
                }
            }
            let store = SqliteEvalStore::open(&cli.db).await?;
            store.get_turn(&turn_id).await?; // 존재 확인
            // **관리자 재라벨은 언제나 human_reviewer** — 사용자 점수를 덮어쓰지 않습니다.
            let r = store
                .rate(&eval::Rating {
                    turn_id: turn_id.clone(),
                    source: eval::Source::HumanReviewer.as_str().into(),
                    rater_id: by.clone(),
                    score: eval::normalize_thumbs(thumbs == "up"),
                    scale: eval::Scale::Thumbs.as_str().into(),
                    labels: label,
                    note,
                    ..Default::default()
                })
                .await?;
            println!("rating_id={} turn_id={} score={:.2} source=human_reviewer", r.id, turn_id, r.score);
        },

        | Cmd::Suites {
            dir,
        } => {
            let suites = Suite::load_dir(&dir)?;
            println!("{:<20} {:<8} PATH", "SUITE", "CASES");
            for s in suites {
                println!("{:<20} {:<8} {}", s.name, s.cases.len(), s.source_path.display());
            }
        },

        | Cmd::Run {
            suite,
            dir,
            fail_under,
            policy,
            principal,
            systems,
            record_turns,
            no_store,
        } => {
            let suites = Suite::load_dir(&dir)?;
            let Some(s) = suites.iter().find(|s| s.name == suite) else {
                bail!("스위트 {suite:?} 을(를) 찾을 수 없습니다 (evalctl suites)");
            };

            let store: Option<Arc<dyn eval::Store>> = if no_store {
                None
            } else {
                Some(Arc::new(SqliteEvalStore::open(&cli.db).await?))
            };

            let runner = build_runner(&policy, &principal, &systems, store, record_turns).await?;
            let rep = runner.run_suite(s).await?;

            println!(
                "run_id={} suite={} pass={} fail={} rate={:.0}%",
                rep.run_id,
                rep.suite,
                rep.pass,
                rep.fail,
                rep.pass_rate() * 100.0
            );
            for r in &rep.results {
                let mark = if r.pass { "PASS" } else { "FAIL" };
                print!("  [{mark}] {}", r.case_id);
                if !r.pass {
                    print!(" {:?}", r.failures);
                    if !r.exec_error.is_empty() {
                        print!(" exec={}", r.exec_error);
                    }
                }
                println!();
            }

            if fail_under > 0.0 && rep.pass_rate() < fail_under {
                bail!("통과율 {:.2} < fail-under {:.2}", rep.pass_rate(), fail_under);
            }
        },

        | Cmd::Runs {
            limit,
        } => {
            let store = SqliteEvalStore::open(&cli.db).await?;
            let runs = store.list_runs(limit).await?;
            if runs.is_empty() {
                println!("(실행 이력 없음)");
                return Ok(());
            }
            println!("{:<20} {:<16} {:<20} {:<6} {:<6} MODEL", "RUN_ID", "SUITE", "STARTED", "PASS", "FAIL");
            for r in runs {
                println!(
                    "{:<20} {:<16} {:<20} {:<6} {:<6} {}",
                    r.run_id,
                    r.suite,
                    r.started_at.map(|x| x.to_rfc3339()).unwrap_or_default(),
                    r.pass_count,
                    r.fail_count,
                    r.model
                );
            }
        },

        | Cmd::Judge {
            turn_id,
            llm,
        } => {
            let store = Arc::new(SqliteEvalStore::open(&cli.db).await?);
            let turn = store.get_turn(&turn_id).await?;
            let auto = Auto {
                rater: Some(store.clone()),
                enable_llm: llm,
                llm: if llm {
                    Some(LlmJudge {
                        provider: open_judge_llm()?,
                        max_tokens: 400,
                    })
                } else {
                    None
                },
                rater_id: "system:judge".into(),
            };
            let report = auto.judge_turn(&turn).await?;
            if let Some(r) = report.rubric {
                println!("{} auto_rubric score={:.2} labels={:?} note={:?}", turn_id, r.score, r.labels, r.note);
            }
            if let Some(r) = report.llm {
                println!("{} auto_llm_judge score={:.2} labels={:?} note={:?}", turn_id, r.score, r.labels, r.note);
            }
        },

        | Cmd::JudgeAll {
            limit,
            since,
            llm,
        } => {
            let store = Arc::new(SqliteEvalStore::open(&cli.db).await?);
            let f = eval::TurnFilter {
                since: Some(chrono::Utc::now() - chrono::Duration::from_std(since.into())?),
                limit,
                ..Default::default()
            };
            let turns = store.query_turns(&f).await?;
            let auto = Auto {
                rater: Some(store.clone()),
                enable_llm: llm,
                llm: if llm {
                    Some(LlmJudge {
                        provider: open_judge_llm()?,
                        max_tokens: 400,
                    })
                } else {
                    None
                },
                rater_id: "system:judge".into(),
            };
            let (mut ok, mut fail) = (0, 0);
            for t in turns {
                match auto.judge_turn(&t).await {
                    | Ok(_) => ok += 1,
                    | Err(e) => {
                        println!("FAIL {}: {e}", t.turn_id);
                        fail += 1;
                    },
                }
            }
            println!("judged ok={ok} fail={fail}");
            if fail > 0 {
                bail!("{fail}개 턴 채점에 실패했습니다");
            }
        },
    }

    Ok(())
}
