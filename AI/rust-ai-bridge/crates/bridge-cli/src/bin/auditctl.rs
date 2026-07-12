//! 감사 로그 · 승인 · 정책 검증 · 운영 현황 CLI.
//!
//! **관리자 CLI 도 게이트웨이와 같은 백엔드를 봐야 합니다.** 게이트웨이가
//! PostgreSQL 로 떴는데 `auditctl` 이 SQLite 를 보면, 관리자가 다른 DB 의
//! 승인을 결정하려다 요청을 찾지 못합니다. 매번 입력하지 않도록
//! `AUDITCTL_POSTGRES_DSN` 환경변수를 봅니다.

use ai_bridge::{SystemsOptions,
                app,
                approval::{self,
                           PostgresApprovalStore,
                           SqliteApprovalStore},
                audit::{self,
                        PostgresLogger,
                        SqliteLogger},
                inventory::Inventory,
                policy,
                workflow::{self,
                           PostgresWorkflowStore,
                           SqliteWorkflowStore}};
use anyhow::{Result,
             bail};
use clap::{Parser,
           Subcommand};
use std::{path::PathBuf,
          sync::Arc};

#[derive(Parser)]
#[command(name = "auditctl", version, about = "감사 로그 · 승인 · 운영 현황")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// SQLite 경로.
    #[arg(long, global = true, default_value = "audit.db")]
    db: PathBuf,
    /// PostgreSQL DSN. 없으면 `AUDITCTL_POSTGRES_DSN` 을 봅니다.
    #[arg(long, global = true)]
    postgres_dsn: Option<String>,
    #[arg(long, global = true, default_value = "config/systems.yaml")]
    systems: PathBuf,
    #[arg(long, global = true, default_value = "config/policies.yaml")]
    policy: PathBuf,
}

#[derive(Subcommand, Clone)]
enum Cmd {
    /// 도구 호출 이력 (기본).
    Log {
        #[arg(long, default_value_t = 20)]
        limit: i64,
        #[arg(long)]
        denied: bool,
        #[arg(long)]
        errors: bool,
        #[arg(long)]
        tool: Option<String>,
        #[arg(long)]
        actor: Option<String>,
        #[arg(long)]
        session: Option<String>,
        /// 예: `1h`, `24h`.
        #[arg(long)]
        since: Option<humantime::Duration>,
    },
    /// 집계.
    Stats {
        #[arg(long, default_value = "tool", value_parser = ["actor", "tool", "system", "session"])]
        by: String,
        #[arg(long, default_value = "24h")]
        since: humantime::Duration,
    },
    /// 감사 해시 체인을 검증합니다.
    Verify,
    /// 승인 대기 목록.
    Approvals {
        #[arg(long, default_value = "pending")]
        status: String,
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    /// 승인합니다. **`--by` 는 필수이며 요청자와 달라야 합니다.**
    Approve {
        id: String,
        #[arg(long)]
        by: String,
        #[arg(long, default_value = "")]
        note: String,
    },
    /// 거부합니다.
    Reject {
        id: String,
        #[arg(long)]
        by: String,
        #[arg(long, default_value = "")]
        note: String,
    },
    /// 보존 기간 현황. **기본은 조회이며 지우지 않습니다.**
    Retention {
        /// 실제로 지웁니다.
        #[arg(long)]
        purge: bool,
        #[arg(long, default_value = "")]
        archive_dir: String,
        #[arg(long, default_value = "")]
        syslog: String,
        /// 아카이브 없이 버립니다 — **되돌릴 수 없습니다.**
        #[arg(long)]
        discard: bool,
        /// 레지스트리에 없는 옛 도구의 보존 일수. **0 = 지우지 않음.**
        #[arg(long, default_value_t = 0)]
        default_days: i64,
    },
    /// 업무 흐름 실행 이력.
    Workflows {
        #[arg(long, default_value = "all")]
        status: String,
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    /// 흐름의 이벤트 로그.
    WorkflowEvents { run_id: String },
    /// 실패한 흐름을 다시 실행 가능하게 만듭니다.
    WorkflowRecover { run_id: String },
    /// 흐름을 취소합니다.
    WorkflowCancel {
        run_id: String,
        #[arg(long, default_value = "operator requested cancellation")]
        reason: String,
    },
    /// 레거시 시스템 상태. **하나라도 죽어 있으면 종료 코드 1.**
    Health {
        #[arg(long, default_value = "")]
        erp_url: String,
    },
    /// 인벤토리와 도구의 일치 여부.
    Inventory,
    /// 정책이 참조하는 것이 전부 실재하는지 검사합니다.
    PolicyCheck,
    /// 정책을 dry-run 합니다. **게이트웨이와 같은 엔진을 씁니다.**
    PolicySimulate {
        #[arg(long)]
        tool: String,
        #[arg(long, default_value = "")]
        roles: String,
        #[arg(long, default_value = "{}")]
        attributes: String,
        #[arg(long, default_value = "{}")]
        args: String,
    },
}

fn resolve_dsn(flag: &Option<String>) -> Option<String> { flag.clone().or_else(|| std::env::var("AUDITCTL_POSTGRES_DSN").ok()).filter(|s| !s.is_empty()) }

async fn open_audit(cli: &Cli) -> Result<Arc<dyn audit::Store>> {
    match resolve_dsn(&cli.postgres_dsn) {
        | Some(dsn) => Ok(Arc::new(PostgresLogger::open(&dsn).await?)),
        | None => Ok(Arc::new(SqliteLogger::open(&cli.db).await?)),
    }
}

async fn open_approvals(cli: &Cli) -> Result<Arc<dyn approval::Store>> {
    match resolve_dsn(&cli.postgres_dsn) {
        | Some(dsn) => Ok(Arc::new(PostgresApprovalStore::open(&dsn).await?)),
        | None => Ok(Arc::new(SqliteApprovalStore::open(&cli.db).await?)),
    }
}

async fn open_workflows(cli: &Cli) -> Result<Arc<dyn workflow::Store>> {
    match resolve_dsn(&cli.postgres_dsn) {
        | Some(dsn) => Ok(Arc::new(PostgresWorkflowStore::open(&dsn).await?)),
        | None => Ok(Arc::new(SqliteWorkflowStore::open(&cli.db).await?)),
    }
}

type Built = (Arc<Inventory>, ai_bridge::registry::Registry, Vec<Arc<dyn ai_bridge::adapter::Adapter>>);

/// 인벤토리·레지스트리를 조립합니다 (정책 검사·헬스체크용).
async fn build(cli: &Cli, erp_url: &str) -> Result<Built> {
    let inv = Arc::new(Inventory::load(&cli.systems)?);
    let opts = SystemsOptions {
        inventory: Some(inv.clone()),
        erp_base_url: erp_url.to_string(),
        ..Default::default()
    };
    let adapters = legacies::build_adapters(&opts, None).await?;
    let reg = app::build_registry(&adapters)?;
    Ok((inv, reg, adapters))
}

fn row(cols: &[String], widths: &[usize]) -> String { cols.iter().zip(widths).map(|(c, w)| format!("{c:<w$}")).collect::<Vec<_>>().join("  ") }

fn dash(s: &str) -> String { if s.is_empty() { "-".into() } else { s.to_string() } }

fn cut(s: &str, n: usize) -> String { s.chars().take(n).collect() }

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cmd = cli.cmd.clone().unwrap_or(Cmd::Log {
        limit: 20,
        denied: false,
        errors: false,
        tool: None,
        actor: None,
        session: None,
        since: None,
    });

    match &cmd {
        | Cmd::Log {
            limit,
            denied,
            errors,
            tool,
            actor,
            session,
            since,
        } => {
            let store = open_audit(&cli).await?;
            let f = audit::Filter {
                tool: tool.clone().unwrap_or_default(),
                actor: actor.clone().unwrap_or_default(),
                session_id: session.clone().unwrap_or_default(),
                decision: if *denied { "denied".into() } else { String::new() },
                errors_only: *errors,
                since: since.map(|d| chrono::Utc::now() - chrono::Duration::from_std(d.into()).unwrap()),
                limit: *limit,
                ..Default::default()
            };
            let rows = store.query(&f).await?;
            if rows.is_empty() {
                println!("기록이 없습니다.");
                return Ok(());
            }
            let w = [4, 9, 16, 24, 8, 9, 7, 8, 30];
            println!(
                "{}",
                row(
                    &["ID", "시각", "주체", "도구", "판단", "승인", "마스킹", "지연ms", "사유"].map(String::from),
                    &w
                )
            );
            for e in rows {
                println!(
                    "{}",
                    row(
                        &[
                            e.id.to_string(),
                            e.timestamp.with_timezone(&chrono::Local).format("%H:%M:%S").to_string(),
                            e.actor.clone(),
                            e.tool.clone(),
                            e.decision.clone(),
                            dash(&e.approval_status),
                            if e.masked { "✓".into() } else { "-".into() },
                            e.latency_ms.to_string(),
                            cut(&e.reason, 30),
                        ],
                        &w
                    )
                );
            }
        },

        | Cmd::Stats {
            by,
            since,
        } => {
            let store = open_audit(&cli).await?;
            let axis = audit::GroupBy::parse(by).unwrap();
            let s = chrono::Utc::now() - chrono::Duration::from_std((*since).into())?;
            let rows = store.stats(axis, Some(s)).await?;
            if rows.is_empty() {
                println!("최근 {since} 동안 기록이 없습니다.");
                return Ok(());
            }
            let w = [24, 6, 6, 6, 8, 12, 12, 10];
            println!(
                "{}",
                row(
                    &[
                        by.clone(),
                        "호출".into(),
                        "거부".into(),
                        "오류".into(),
                        "거부율".into(),
                        "평균지연ms".into(),
                        "최대지연ms".into(),
                        "비용".into()
                    ],
                    &w
                )
            );
            for st in rows {
                let rate = if st.calls == 0 { 0.0 } else { st.denied as f64 / st.calls as f64 * 100.0 };
                println!(
                    "{}",
                    row(
                        &[
                            st.key,
                            st.calls.to_string(),
                            st.denied.to_string(),
                            st.errors.to_string(),
                            format!("{rate:.1}%"),
                            format!("{:.1}", st.avg_latency_ms),
                            st.max_latency_ms.to_string(),
                            st.cost_micros.to_string(),
                        ],
                        &w
                    )
                );
            }
        },

        | Cmd::Verify => {
            match resolve_dsn(&cli.postgres_dsn) {
                | Some(dsn) => PostgresLogger::open(&dsn).await?.verify_integrity().await?,
                | None => SqliteLogger::open(&cli.db).await?.verify_integrity().await?,
            }
            println!("감사 해시 체인이 온전합니다 — 행 삭제·수정·체인 단절이 없습니다.");
        },

        | Cmd::Approvals {
            status,
            limit,
        } => {
            let store = open_approvals(&cli).await?;
            let filter = if status == "all" { None } else { approval::Status::parse(status) };
            let rows = store.list(filter, *limit).await?;
            if rows.is_empty() {
                println!("상태가 {status:?} 인 승인 요청이 없습니다.");
                return Ok(());
            }
            let w = [18, 16, 16, 24, 10, 14, 24];
            println!(
                "{}",
                row(&["요청ID", "요청시각", "주체", "도구", "상태", "결정자", "인자"].map(String::from), &w)
            );
            for r in &rows {
                println!(
                    "{}",
                    row(
                        &[
                            r.id.clone(),
                            r.requested_at.with_timezone(&chrono::Local).format("%m-%d %H:%M:%S").to_string(),
                            r.actor.clone(),
                            r.tool.clone(),
                            r.status.to_string(),
                            dash(&r.decided_by),
                            cut(&format!("{:?}", r.args), 24),
                        ],
                        &w
                    )
                );
            }
            if filter == Some(approval::Status::Pending) {
                println!("\n승인: auditctl approve <요청ID> --by <이름>");
                println!("      결정자는 요청자와 달라야 합니다. 승인 후 유효기간 안에 실행해야 합니다.");
            }
        },

        | Cmd::Approve {
            id,
            by,
            note,
        }
        | Cmd::Reject {
            id,
            by,
            note,
        } => {
            let approve = matches!(cmd, Cmd::Approve { .. });
            if by.is_empty() {
                bail!("--by 로 결정자 이름을 지정해야 합니다(감사 추적을 위해 필수)");
            }
            let store = open_approvals(&cli).await?;
            match store.decide(id, approve, by, note).await {
                | Ok(r) => {
                    println!("요청 {} → {} (결정자: {by})", r.id, if approve { "승인" } else { "거부" });
                    println!("  도구: {}  주체: {}  인자: {:?}", r.tool, r.actor, r.args);
                    if approve {
                        println!("\n같은 인자로 도구를 다시 호출하면 이번 한 번 실행됩니다.");
                        if let Some(exp) = r.expires_at {
                            println!(
                                "유효 기간: {:?} ({} 까지). 지나면 승인이 만료되고 새 요청이 만들어집니다.",
                                r.ttl,
                                exp.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")
                            );
                        }
                    }
                },
                // **요청자는 자기 요청을 결정할 수 없습니다** — 저장소가 막습니다.
                | Err(approval::Error::SelfApproval(who)) => bail!("요청자는 자기 요청을 결정할 수 없습니다. 다른 사람이 검토해야 합니다(요청자: {who})"),
                | Err(approval::Error::NotPending) => bail!("승인 요청 {id:?} 은(는) 이미 결정되었습니다"),
                | Err(approval::Error::NotFound) => bail!("승인 요청 {id:?} 을(를) 찾을 수 없습니다. 목록: auditctl approvals"),
                | Err(e) => bail!("{e}"),
            }
        },

        | Cmd::Retention {
            purge,
            archive_dir,
            syslog,
            discard,
            default_days,
        } => {
            let store = open_audit(&cli).await?;
            let (_, reg, _) = build(&cli, "").await?;

            let mut pol = app::retention_policy(&reg);
            pol.default = *default_days;
            let oldest = store.oldest().await?;
            let now = chrono::Utc::now();

            let w = [24, 10, 20, 10, 16];
            println!("{}", row(&["도구", "보존(일)", "가장 오래된 기록", "경과(일)", "상태"].map(String::from), &w));
            for s in reg.specs() {
                let days = s.log_retention_days;
                let (o, age, state) = match oldest.get(&s.name) {
                    | Some(f) => {
                        let a = (now - *f).num_days();
                        let over = days > 0 && a > days;
                        (
                            f.format("%Y-%m-%d").to_string(),
                            a.to_string(),
                            if over { "보존 기간 초과" } else { "정상" }.to_string(),
                        )
                    },
                    | None => ("-".into(), "-".into(), "기록 없음".into()),
                };
                println!(
                    "{}",
                    row(&[s.name, if days == 0 { "영구".into() } else { days.to_string() }, o, age, state,], &w)
                );
            }

            if !purge {
                // **기본은 조회입니다.** 감사 기록 삭제는 되돌릴 수 없으므로, 무엇이
                // 지워질지 먼저 보여주고 명시적으로 요청할 때만 지웁니다.
                println!("\n삭제하려면 --purge 와 아카이브 대상(--archive-dir · --syslog · --discard)을 주세요.");
                return Ok(());
            }

            // **아카이브는 선택이 아닙니다.**
            let exporter = audit::build_exporter(archive_dir, syslog, *discard)?;
            match store.purge(&pol, now, exporter.as_ref()).await {
                | Ok(p) => {
                    if p.deleted == 0 {
                        println!("\n보존 기간이 지난 기록이 없습니다.");
                    } else {
                        println!("\n{}건을 아카이브 후 삭제했습니다.", p.deleted);
                        for (tool, n) in &p.by_tool {
                            println!("  {tool}: {n}건");
                        }
                    }
                    if !p.skipped.is_empty() {
                        println!("영구 보존(건너뜀): {}", p.skipped.join(", "));
                    }
                },
                | Err(e) => bail!("아카이브 실패로 삭제를 중단했습니다: {e}"),
            }
        },

        | Cmd::Workflows {
            status,
            limit,
        } => {
            let store = open_workflows(&cli).await?;
            let filter = if status == "all" { None } else { workflow::Status::parse(status) };
            let runs = store.list(filter, *limit).await?;
            if runs.is_empty() {
                println!("해당 상태의 업무 흐름이 없습니다.");
                return Ok(());
            }
            let w = [28, 12, 14, 10, 10, 44];
            println!("{}", row(&["실행ID", "흐름", "상태", "완료단계", "갱신", "사유"].map(String::from), &w));
            for r in runs {
                // **보상 실패는 사람이 봐야 합니다.**
                let reason = if !r.compensate_error.is_empty() {
                    format!("보상 실패(사람 확인 필요): {}", r.compensate_error)
                } else {
                    r.error.clone()
                };
                println!(
                    "{}",
                    row(
                        &[
                            r.id,
                            r.name,
                            r.status.to_string(),
                            r.completed.len().to_string(),
                            r.updated_at
                                .map(|t| t.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
                                .unwrap_or_else(|| "-".into()),
                            cut(&reason, 44),
                        ],
                        &w
                    )
                );
            }
        },

        | Cmd::WorkflowEvents {
            run_id,
        } => {
            let store = open_workflows(&cli).await?;
            let events = store.events(run_id).await?;
            if events.is_empty() {
                println!("이벤트가 없습니다.");
                return Ok(());
            }
            let w = [16, 24, 20, 5, 16, 6, 30];
            println!("{}", row(&["시각", "유형", "단계", "시도", "worker", "fence", "메시지"].map(String::from), &w));
            for e in events {
                println!(
                    "{}",
                    row(
                        &[
                            e.at.with_timezone(&chrono::Local).format("%m-%d %H:%M:%S").to_string(),
                            e.r#type,
                            dash(&e.step),
                            e.attempt.to_string(),
                            dash(&e.worker),
                            e.fencing_token.to_string(),
                            cut(&e.message, 30),
                        ],
                        &w
                    )
                );
            }
        },

        | Cmd::WorkflowRecover {
            run_id,
        } => {
            let store = open_workflows(&cli).await?;
            let run = workflow::Engine::new(store).recover(run_id).await?;
            println!("{}: {}", run.id, run.status);
            println!("완료 단계를 비우고 복구 세대를 올렸습니다(recovery-{}).", run.recovery_count);
        },

        | Cmd::WorkflowCancel {
            run_id,
            reason,
        } => {
            let store = open_workflows(&cli).await?;
            let run = workflow::Engine::new(store).cancel(run_id, reason).await?;
            println!("{}: {}", run.id, run.status);
        },

        | Cmd::Health {
            erp_url,
        } => {
            let (inv, _, adapters) = build(&cli, erp_url).await?;
            let rows = app::check_health(&adapters, std::time::Duration::from_secs(5)).await;
            let w = [12, 8, 10, 12, 16, 30];
            println!("{}", row(&["시스템", "상태", "응답ms", "장애영향도", "담당부서", "오류"].map(String::from), &w));
            let mut unhealthy = 0;
            for h in &rows {
                let sys = inv.lookup(&h.system);
                if !h.healthy {
                    unhealthy += 1;
                }
                println!(
                    "{}",
                    row(
                        &[
                            h.system.clone(),
                            if h.healthy { "정상".into() } else { "장애".into() },
                            h.latency.as_millis().to_string(),
                            sys.as_ref().map(|s| s.failure_impact.to_string()).unwrap_or_default(),
                            sys.map(|s| s.owner_team).unwrap_or_default(),
                            cut(&h.error, 30),
                        ],
                        &w
                    )
                );
            }
            if unhealthy > 0 {
                // 헬스체크 스크립트에 그대로 쓸 수 있도록 종료 코드로 알립니다.
                bail!("{unhealthy}개 시스템이 응답하지 않습니다");
            }
        },

        | Cmd::Inventory => {
            let (inv, reg, _) = build(&cli, "").await?;
            let w = [12, 22, 12, 24, 20, 16];
            println!("{}", row(&["시스템", "이름", "인터페이스", "민감도", "기능", "담당부서"].map(String::from), &w));
            for s in inv.systems() {
                println!(
                    "{}",
                    row(
                        &[
                            s.name.clone(),
                            s.display_name.clone(),
                            s.interface.to_string(),
                            s.data_sensitivity.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(","),
                            s.capabilities.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","),
                            s.owner_team.clone(),
                        ],
                        &w
                    )
                );
            }
            app::validate_inventory(&reg, &inv)?;
            for s in app::unused_systems(&reg, &inv) {
                println!("경고: 시스템 {s:?} 에 등록된 도구가 없습니다");
            }
            println!("\n시스템 {}개, 도구 {}개 — 인벤토리와 일치합니다.", inv.len(), reg.len());
        },

        | Cmd::PolicyCheck => {
            let (inv, reg, _) = build(&cli, "").await?;
            let engine = policy::Engine::load(&cli.policy)?;
            policy::validate_references(&engine.snapshot(), &reg, &inv)?;
            let (version, digest) = engine.version();
            println!(
                "정책 {version} ({}) — 시스템 {}개, 도구 {}개와 참조가 일치합니다.",
                &digest[.. 12],
                inv.len(),
                reg.len()
            );
        },

        // **게이트웨이와 같은 정책 엔진을 dry-run 합니다** — 실제 호출이나 감사 데이터를
        // 만들지 않습니다. 같은 입력으로 Go 판과 Rust 판의 판정을 대조할 수 있습니다.
        | Cmd::PolicySimulate {
            tool,
            roles,
            attributes,
            args,
        } => {
            let (inv, reg, _) = build(&cli, "").await?;
            let engine = policy::Engine::load(&cli.policy)?;
            policy::validate_references(&engine.snapshot(), &reg, &inv)?;

            let Some(t) = reg.lookup(tool) else {
                bail!("unknown tool {tool:?}")
            };
            let attrs: serde_json::Map<String, serde_json::Value> = serde_json::from_str(attributes)?;
            let call_args: serde_json::Map<String, serde_json::Value> = serde_json::from_str(args)?;

            let id = ai_bridge::auth::Identity {
                user_id: "policy-simulator".into(),
                roles: roles.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                attributes: attrs.into_iter().collect(),
                ..Default::default()
            };

            let d = engine.evaluate(&id, &t.spec, &call_args);
            println!(
                "allowed={} approval_required={} rule={} matched={:?} reason={} obligations={:?}",
                d.allowed, d.approval_required, d.rule_id, d.matched_rules, d.reason, d.obligations
            );
        },
    }

    Ok(())
}
