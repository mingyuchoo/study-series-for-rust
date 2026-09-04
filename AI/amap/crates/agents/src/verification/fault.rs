//! Fault Injection Agent (design §16): DB/API timeouts, duplicate messages, partial commits …
use super::{build_engine, legacy_oracle, next_system};
use amap_domain::*;
use amap_orchestrator::RunConfig;
use amap_replay::{ExecOptions, SystemUnderTest};
use serde_json::{json, Value};

const ACCEPTABLE_STATUS: &[&str] = &[
    "RETRY_SCHEDULED",
    "ROLLED_BACK",
    "DUPLICATE_IGNORED",
    "FAILED",
    "REJECTED",
    "DEGRADED",
];

pub async fn run(
    scenarios: &[TestScenario],
    cfg: &RunConfig,
    function_id: &FunctionId,
) -> Result<(Vec<VerificationResult>, Value), String> {
    let engine = build_engine(cfg)?;
    let next = next_system(cfg)?;
    let oracle = legacy_oracle(cfg);
    let sample: Vec<TestScenario> = scenarios
        .iter()
        .filter(|s| s.origin == ScenarioOrigin::GoldenMaster)
        .take(5)
        .cloned()
        .collect();
    let mut results = Vec::new();
    for fault in &cfg.faults {
        let cases: Vec<TestScenario> = sample
            .iter()
            .map(|s| TestScenario {
                id: ScenarioId::new(format!("FAULT-{}-{}", fault, s.id.0)),
                expected_output: None,
                expected_state_change: None,
                expected_events: None,
                expected_external_calls: None,
                expected_timing_ms: None,
                origin: ScenarioOrigin::Fault,
                ..s.clone()
            })
            .collect();
        let opts = ExecOptions {
            fault: Some(fault.clone()),
            ..Default::default()
        };
        let mut res = engine
            .replay(
                &next,
                oracle.as_ref().map(|o| o as &dyn SystemUnderTest),
                &cases,
                VerificationKind::Fault,
                &opts,
            )
            .await
            .map_err(|e| e.to_string())?;
        if oracle.is_none() {
            // Without a legacy oracle, require an explicit, business-appropriate recovery status and intact invariants.
            for r in res.iter_mut() {
                let status = r.details["actual_output"]["status"].as_str().unwrap_or("");
                if !ACCEPTABLE_STATUS.contains(&status) {
                    r.passed = false;
                    r.differences.push(Difference {
                        path: "output.status".into(),
                        expected: json!(ACCEPTABLE_STATUS),
                        actual: json!(status),
                        comparator: "fault-policy".into(),
                        message: format!("no recovery status under fault `{fault}`"),
                    });
                }
            }
        }
        results.extend(res);
    }
    let passed = results.iter().filter(|r| r.passed).count();
    let cases = results.len();
    let _ = function_id;
    Ok((
        results,
        json!({ "faults": cfg.faults, "cases": cases, "passed": passed }),
    ))
}
