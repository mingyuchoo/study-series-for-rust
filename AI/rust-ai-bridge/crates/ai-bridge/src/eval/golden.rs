//! 골든셋 회귀 — **LLM 없이** 게이트웨이에 도구를 스크립트로 태웁니다.
//!
//! 결정적이고 싸며 CI 에 그대로 걸 수 있습니다. LLM judge 와는 목적이 다릅니다
//! — 이쪽은 "게이트웨이가 여전히 같은 판정을 내리는가"를 봅니다.

use super::{Outcome,
            Rating,
            RunResult,
            Scale,
            Source,
            Store,
            ToolStep,
            Turn,
            args_digest};
use crate::{auth::{Enricher,
                   Identity,
                   RequestContext},
            gateway::{Call,
                      Gateway}};
use anyhow::{Result,
             anyhow,
             bail};
use serde::Deserialize;
use serde_json::{Map,
                 Value};
use std::{path::{Path,
                 PathBuf},
          sync::Arc};

/// 검사할 것.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    #[serde(default)]
    pub reply_contains: Vec<String>,
    #[serde(default)]
    pub reply_not_contains: Vec<String>,
    #[serde(default)]
    pub reply_matches: Vec<String>,
    /// 순서 무관.
    #[serde(default)]
    pub tools_called: Vec<String>,
    /// **부분 수열**이어야 합니다 (연속일 필요는 없음).
    #[serde(default)]
    pub tools_called_ordered: Vec<String>,
    #[serde(default)]
    pub tools_not_called: Vec<String>,
    #[serde(default)]
    pub outcome: String,
    /// 도구 → `allowed` | `denied` | `dry_run`. **마지막 호출**을 봅니다.
    #[serde(default)]
    pub tool_decisions: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub max_cost_micros: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub tool: String,
    #[serde(default)]
    pub args: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub principal: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub steps: Vec<Step>,
    /// 비면 마지막 도구 출력에서 합성합니다.
    #[serde(default)]
    pub reply: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub expect: Expect,
    /// **이 케이스만** L4 허용 게이트웨이로 보냅니다.
    #[serde(default)]
    pub allow_high_risk: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    #[serde(rename = "suite", default)]
    pub name: String,
    pub cases: Vec<Case>,
    #[serde(skip)]
    pub source_path: PathBuf,
}

impl Suite {
    pub fn load_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let mut s: Suite = serde_norway::from_str(&data).map_err(|e| anyhow!("parse suite {}: {e}", path.display()))?;
        if s.name.is_empty() {
            s.name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        }
        s.source_path = path.to_path_buf();
        s.validate()?;
        Ok(s)
    }

    pub fn load_dir(dir: &Path) -> Result<Vec<Self>> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir)? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x == "yaml" || x == "yml") {
                out.push(Self::load_file(&p)?);
            }
        }
        if out.is_empty() {
            bail!("{}: 스위트 파일이 없습니다", dir.display());
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn validate(&self) -> Result<()> {
        if self.cases.is_empty() {
            bail!("suite {:?}: 케이스가 없습니다", self.name);
        }
        let mut seen = std::collections::HashSet::new();
        for c in &self.cases {
            if c.id.is_empty() {
                bail!("suite {:?}: case id 가 필요합니다", self.name);
            }
            if !seen.insert(&c.id) {
                bail!("suite {:?}: 중복 case id {:?}", self.name, c.id);
            }
            if c.principal.is_empty() {
                bail!("case {:?}: principal 이 필요합니다", c.id);
            }
            if c.steps.is_empty() && c.reply.is_empty() {
                bail!("case {:?}: steps 또는 reply 가 필요합니다", c.id);
            }
            for s in &c.steps {
                if s.tool.is_empty() {
                    bail!("case {:?}: step tool 이 필요합니다", c.id);
                }
            }
        }
        Ok(())
    }
}

/// 관찰된 것.
#[derive(Debug, Clone, Default)]
pub struct Observed {
    pub reply: String,
    pub trail: Vec<ToolStep>,
    pub outcome: String,
    pub cost_micros: i64,
    pub exec_error: String,
}

#[derive(Debug, Clone, Default)]
pub struct CheckResult {
    pub pass: bool,
    pub failures: Vec<String>,
}

/// 기대와 관찰을 대조합니다. **모든 실패를 모읍니다** — 첫 실패에서 멈추지
/// 않습니다.
pub fn check(exp: &Expect, obs: &Observed) -> CheckResult {
    let mut f: Vec<String> = Vec::new();
    let names: Vec<&str> = obs.trail.iter().map(|t| t.name.as_str()).collect();

    for s in &exp.reply_contains {
        if !obs.reply.contains(s) {
            f.push(format!("reply_contains {s:?} 이(가) 없습니다"));
        }
    }
    for s in &exp.reply_not_contains {
        if obs.reply.contains(s) {
            f.push(format!("reply_not_contains {s:?} 이(가) 나타났습니다"));
        }
    }
    for p in &exp.reply_matches {
        match regex::Regex::new(p) {
            | Ok(re) =>
                if !re.is_match(&obs.reply) {
                    f.push(format!("reply_matches {p:?} 이(가) 맞지 않습니다"));
                },
            | Err(e) => f.push(format!("reply_matches {p:?} 정규식 오류: {e}")),
        }
    }
    for t in &exp.tools_called {
        if !names.contains(&t.as_str()) {
            f.push(format!("tools_called {t:?} 이(가) 호출되지 않았습니다"));
        }
    }
    for t in &exp.tools_not_called {
        if names.contains(&t.as_str()) {
            f.push(format!("tools_not_called {t:?} 이(가) 호출되었습니다"));
        }
    }
    if !exp.tools_called_ordered.is_empty() && !is_subsequence(&exp.tools_called_ordered, &names) {
        f.push(format!(
            "tools_called_ordered {:?} 순서가 맞지 않습니다 (실제: {names:?})",
            exp.tools_called_ordered
        ));
    }
    if !exp.outcome.is_empty() && exp.outcome != obs.outcome {
        f.push(format!("outcome: 기대 {:?}, 실제 {:?}", exp.outcome, obs.outcome));
    }
    for (tool, want) in &exp.tool_decisions {
        // **마지막 호출**의 판정을 봅니다.
        match obs.trail.iter().rev().find(|t| &t.name == tool) {
            | Some(step) if &step.decision == want => {},
            | Some(step) => f.push(format!("tool_decisions {tool:?}: 기대 {want:?}, 실제 {:?}", step.decision)),
            | None => f.push(format!("tool_decisions {tool:?}: 호출되지 않았습니다")),
        }
    }
    if exp.max_cost_micros > 0 && obs.cost_micros > exp.max_cost_micros {
        f.push(format!("max_cost_micros: {} > {}", obs.cost_micros, exp.max_cost_micros));
    }

    CheckResult {
        pass: f.is_empty(),
        failures: f,
    }
}

/// `want` 가 `got` 의 (연속일 필요 없는) 부분 수열인가.
fn is_subsequence(want: &[String], got: &[&str]) -> bool {
    let mut i = 0;
    for g in got {
        if i < want.len() && &want[i] == g {
            i += 1;
        }
    }
    i == want.len()
}

/// 실행 보고.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub run_id: String,
    pub suite: String,
    pub pass: usize,
    pub fail: usize,
    pub results: Vec<CaseReport>,
}

impl Report {
    pub fn pass_rate(&self) -> f64 {
        let total = self.pass + self.fail;
        if total == 0 {
            return 1.0;
        }
        self.pass as f64 / total as f64
    }
}

#[derive(Debug, Clone)]
pub struct CaseReport {
    pub case_id: String,
    pub pass: bool,
    pub failures: Vec<String>,
    pub exec_error: String,
    pub reply: String,
    pub trail: Vec<ToolStep>,
}

/// 골든셋 러너.
pub struct Runner {
    /// 일반 게이트웨이.
    pub gateway: Arc<Gateway>,
    /// `allow_high_risk` 케이스용.
    pub gateway_high: Arc<Gateway>,
    pub principals: crate::auth::Directory,
    pub store: Option<Arc<dyn Store>>,
    pub record_turns: bool,
    pub git_sha: String,
    pub model: String,
}

/// **시계를 고정합니다.** `business-hours-only` 규칙은 주말을 업무시간이
/// 아니라고 보므로, 실제 시계를 쓰면 토·일요일에 모든 케이스가
/// `permission_denied` 로 실패합니다. 2026-07-13 은 월요일입니다.
fn fixed_clock() -> chrono::DateTime<chrono::Utc> {
    use chrono::TimeZone as _;
    chrono::Utc.with_ymd_and_hms(2026, 7, 13, 10, 0, 0).unwrap()
}

/// **에이전트가 멈추는 오류** — 이 코드가 나오면 남은 단계를 실행하지 않습니다.
fn halts(code: &str) -> bool { matches!(code, "budget_exceeded" | "approval_rejected" | "approval_error") }

impl Runner {
    pub async fn run_suite(&self, suite: &Suite) -> Result<Report> {
        let run_id = format!("run_{}", super::new_id("").trim_start_matches('_'));
        let started = chrono::Utc::now();

        if let Some(s) = &self.store {
            s.record_run(&super::Run {
                run_id: run_id.clone(),
                suite: suite.name.clone(),
                started_at: Some(started),
                git_sha: self.git_sha.clone(),
                model: self.model.clone(),
                ..Default::default()
            })
            .await?;
        }

        let mut report = Report {
            run_id: run_id.clone(),
            suite: suite.name.clone(),
            ..Default::default()
        };

        for c in &suite.cases {
            let obs = self.run_case(c).await;
            let res = check(&c.expect, &obs);

            if res.pass {
                report.pass += 1;
            } else {
                report.fail += 1;
            }

            report.results.push(CaseReport {
                case_id: c.id.clone(),
                pass: res.pass,
                failures: res.failures.clone(),
                exec_error: obs.exec_error.clone(),
                reply: obs.reply.clone(),
                trail: obs.trail.clone(),
            });

            if let Some(s) = &self.store {
                s.add_run_result(&RunResult {
                    run_id: run_id.clone(),
                    case_id: c.id.clone(),
                    pass: res.pass,
                    score: if res.pass { 1.0 } else { 0.0 },
                    actual_reply: obs.reply.clone(),
                    trail: obs.trail.clone(),
                    error: res.failures.join("; "),
                    ..Default::default()
                })
                .await?;

                if self.record_turns {
                    let turn = s
                        .record_turn(&Turn {
                            actor: c.principal.clone(),
                            channel: "golden".into(),
                            model: self.model.clone(),
                            prompt: c.prompt.clone(),
                            reply: obs.reply.clone(),
                            tool_trail: obs.trail.clone(),
                            outcome: obs.outcome.clone(),
                            cost_micros: obs.cost_micros,
                            ..Default::default()
                        })
                        .await?;
                    s.rate(&Rating {
                        turn_id: turn.turn_id,
                        source: Source::Golden.as_str().into(),
                        rater_id: "system:golden".into(),
                        score: if res.pass { 1.0 } else { 0.0 },
                        scale: Scale::BinaryPass.as_str().into(),
                        ..Default::default()
                    })
                    .await?;
                }
            }
        }

        if let Some(s) = &self.store {
            s.finish_run(&run_id, chrono::Utc::now(), report.pass as i64, report.fail as i64).await?;
        }

        Ok(report)
    }

    async fn run_case(&self, c: &Case) -> Observed {
        let mut obs = Observed {
            outcome: Outcome::Completed.as_str().into(),
            ..Default::default()
        };

        let Some(base) = self.principals.lookup(&c.principal) else {
            obs.exec_error = format!("주체 {:?} 을(를) 찾을 수 없습니다", c.principal);
            obs.outcome = Outcome::Error.as_str().into();
            return obs;
        };

        // 환경 속성을 고정된 시계로 계산합니다.
        let e = Enricher {
            internal_prefixes: crate::auth::parse_prefixes(&["10.0.0.0/8".into()]).unwrap_or_default(),
            default_llm_destination: "internal".into(),
            default_business_purpose: "sales_followup".into(),
            ..Default::default()
        };
        let mut rc = RequestContext {
            now: Some(fixed_clock()),
            session_id: format!("golden-{}", c.id),
            ..Default::default()
        };
        rc.set(crate::auth::REMOTE_ADDR_HEADER, "10.1.2.3:5555");
        let id: Identity = e.enrich(&base, &rc);

        let gw = if c.allow_high_risk { &self.gateway_high } else { &self.gateway };

        let mut last_output: Option<Value> = None;
        let mut last_summary = String::new();

        for step in &c.steps {
            let call = Call {
                tool: step.tool.clone(),
                args: step.args.clone(),
                prompt: c.prompt.clone(),
                ..Default::default()
            };

            match gw.handle(&id, call).await {
                | Ok(r) => {
                    let decision = if r.dry_run { "dry_run" } else { "allowed" };
                    obs.trail.push(ToolStep {
                        name: step.tool.clone(),
                        args_digest: args_digest(&step.args),
                        decision: decision.into(),
                        audit_request_id: r.request_id.clone(),
                        ..Default::default()
                    });
                    if r.dry_run {
                        last_summary = r.summary.clone();
                    } else {
                        last_output = Some(Value::Object(r.data.clone()));
                    }
                },
                | Err(err) => {
                    obs.trail.push(ToolStep {
                        name: step.tool.clone(),
                        args_digest: args_digest(&step.args),
                        decision: "denied".into(),
                        error_code: err.code.clone(),
                        ..Default::default()
                    });
                    // 일부 오류는 에이전트를 멈춥니다.
                    if halts(&err.code) {
                        obs.outcome = Outcome::Halted.as_str().into();
                        obs.exec_error = err.to_string();
                        break;
                    }
                    // 그 밖의 오류는 기록만 하고 다음 단계로 갑니다 — 정책
                    // 거부는 **정상적으로 완료된
                    // 턴**입니다(러너 오류가 아닙니다).
                },
            }
        }

        // 답변을 합성합니다.
        obs.reply = if !c.reply.is_empty() {
            c.reply.clone()
        } else if !last_summary.is_empty() {
            last_summary
        } else if let Some(o) = last_output {
            serde_json::to_string(&o).unwrap_or_default()
        } else {
            String::new()
        };

        obs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(trail: &[(&str, &str)], reply: &str) -> Observed {
        Observed {
            reply: reply.into(),
            trail: trail
                .iter()
                .map(|(n, d)| ToolStep {
                    name: (*n).into(),
                    decision: (*d).into(),
                    ..Default::default()
                })
                .collect(),
            outcome: "completed".into(),
            ..Default::default()
        }
    }

    #[test]
    fn check_collects_all_failures_not_just_the_first() {
        let exp = Expect {
            reply_contains: vec!["paid".into(), "INV-1".into()],
            tools_called: vec!["missing_tool".into()],
            ..Default::default()
        };
        let r = check(&exp, &obs(&[], "nothing"));
        assert!(!r.pass);
        // 첫 실패에서 멈추면 나머지를 못 봅니다.
        assert_eq!(r.failures.len(), 3);
    }

    #[test]
    fn ordered_tools_must_be_a_subsequence_not_contiguous() {
        let exp = Expect {
            tools_called_ordered: vec!["a".into(), "c".into()],
            ..Default::default()
        };
        // a, b, c — a 뒤에 c 가 오므로 부분 수열입니다.
        assert!(check(&exp, &obs(&[("a", "allowed"), ("b", "allowed"), ("c", "allowed")], "")).pass);
        // c 가 a 앞에 오면 실패합니다.
        assert!(!check(&exp, &obs(&[("c", "allowed"), ("a", "allowed")], "")).pass);
    }

    #[test]
    fn tool_decisions_look_at_the_last_occurrence() {
        let exp = Expect {
            tool_decisions: std::collections::HashMap::from([("t".to_string(), "allowed".to_string())]),
            ..Default::default()
        };
        // 처음엔 dry_run, 승인 뒤 allowed — 마지막이 기준입니다.
        assert!(check(&exp, &obs(&[("t", "dry_run"), ("t", "allowed")], "")).pass);
        assert!(!check(&exp, &obs(&[("t", "allowed"), ("t", "dry_run")], "")).pass);
    }

    #[test]
    fn a_missing_tool_in_tool_decisions_is_a_failure() {
        let exp = Expect {
            tool_decisions: std::collections::HashMap::from([("t".to_string(), "allowed".to_string())]),
            ..Default::default()
        };
        assert!(!check(&exp, &obs(&[], "")).pass);
    }

    #[test]
    fn max_cost_is_only_checked_when_set() {
        let mut o = obs(&[], "");
        o.cost_micros = 5000;
        assert!(check(&Expect::default(), &o).pass);

        let exp = Expect {
            max_cost_micros: 1000,
            ..Default::default()
        };
        assert!(!check(&exp, &o).pass);
    }

    #[test]
    fn only_specific_errors_halt_the_agent() {
        assert!(halts("budget_exceeded"));
        assert!(halts("approval_rejected"));
        // 정책 거부는 **정상적으로 완료된 턴**입니다 — 러너 오류가 아닙니다.
        assert!(!halts("permission_denied"));
        assert!(!halts("adapter_error"));
    }

    #[test]
    fn loads_the_real_suites() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval/suites");
        let suites = Suite::load_dir(&dir).expect("eval/suites 를 읽을 수 있어야 합니다");
        assert!(suites.len() >= 3);
        for s in &suites {
            assert!(!s.cases.is_empty(), "suite {} 에 케이스가 없습니다", s.name);
        }
    }
}
