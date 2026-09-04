//! Concurrency Agent (design §17): enumerate interleavings of concurrent transactions and check
//! the next system's transaction semantics (serialisable outcome, no lost update, no negative balance).
use super::{legacy_oracle, next_system};
use amap_domain::*;
use amap_orchestrator::RunConfig;
use amap_replay::{ExecOptions, ReplayCase, SystemUnderTest};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// All interleavings of per-transaction step sequences (order within a transaction preserved).
pub fn interleavings(txns: &[(String, Vec<String>)], cap: usize) -> Vec<Vec<(String, String)>> {
    fn rec(
        txns: &[(String, Vec<String>)],
        pos: &mut Vec<usize>,
        cur: &mut Vec<(String, String)>,
        out: &mut Vec<Vec<(String, String)>>,
        cap: usize,
    ) {
        if out.len() >= cap {
            return;
        }
        if pos.iter().enumerate().all(|(i, &p)| p == txns[i].1.len()) {
            out.push(cur.clone());
            return;
        }
        for i in 0..txns.len() {
            if pos[i] < txns[i].1.len() {
                cur.push((txns[i].0.clone(), txns[i].1[pos[i]].clone()));
                pos[i] += 1;
                rec(txns, pos, cur, out, cap);
                pos[i] -= 1;
                cur.pop();
            }
        }
    }
    let mut out = Vec::new();
    rec(
        txns,
        &mut vec![0; txns.len()],
        &mut Vec::new(),
        &mut out,
        cap,
    );
    out
}

pub async fn run(
    cfg: &RunConfig,
    function_id: &FunctionId,
) -> Result<(Vec<VerificationResult>, Value), String> {
    let Some(template) = &cfg.concurrency_template else {
        return Ok((
            vec![],
            json!({ "schedules": 0, "reason": "no concurrency template" }),
        ));
    };
    let next = next_system(cfg)?;
    let initial_state = template["initial_state"].clone();
    let txns: Vec<Value> = template["transactions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let steps: Vec<(String, Vec<String>)> = txns
        .iter()
        .map(|t| {
            (
                t["id"].as_str().unwrap_or("T").to_string(),
                vec!["read".into(), "write".into()],
            )
        })
        .collect();
    let schedules = interleavings(&steps, 200);
    let initial_balance = initial_state["balance"].as_f64().unwrap_or(0.0);

    // Serial schedules define the allowed (serialisable) outcomes.
    let serial: Vec<Vec<(String, String)>> = {
        let mut v = Vec::new();
        let n = steps.len();
        let mut order: Vec<usize> = (0..n).collect();
        let steps_ref = &steps;
        permute(&mut order, 0, &mut |o: &[usize]| {
            let mut sched: Vec<(String, String)> = Vec::new();
            for &i in o {
                for s in &steps_ref[i].1 {
                    sched.push((steps_ref[i].0.clone(), s.clone()));
                }
            }
            v.push(sched);
        });
        v
    };
    // Allowed outcomes come from the legacy oracle when available (differential), else from the next system itself.
    let oracle = legacy_oracle(cfg);
    let reference: &dyn SystemUnderTest = match &oracle {
        Some(o) => o,
        None => &next,
    };
    let mut allowed: BTreeSet<i64> = BTreeSet::new();
    for s in &serial {
        let case = mk_case("serial", &initial_state, &txns, s);
        if let Ok(e) = reference.execute(&case).await {
            if let Some(b) = e.output["final_balance"].as_f64() {
                allowed.insert(b.round() as i64);
            }
        }
    }

    let cases: Vec<ReplayCase> = schedules
        .iter()
        .enumerate()
        .map(|(i, s)| mk_case(&format!("CONC-{i:04}"), &initial_state, &txns, s))
        .collect();
    let execs = next.execute_batch(&cases).await;
    let oracle_execs = match &oracle {
        Some(o) => o
            .execute_batch(&cases)
            .await
            .into_iter()
            .map(|r| r.ok())
            .collect::<Vec<_>>(),
        None => cases.iter().map(|_| None).collect(),
    };
    let mut results = Vec::new();
    for (i, (case, exec)) in cases.iter().zip(execs).enumerate() {
        let mut diffs = Vec::new();
        let details = match exec {
            Ok(e) => {
                let final_balance = e.output["final_balance"].as_f64().unwrap_or(f64::NAN);
                let committed: f64 = e.output["outcomes"]
                    .as_object()
                    .map(|o| {
                        o.iter()
                            .filter(|(_, v)| v.as_str() == Some("COMMITTED"))
                            .map(|(k, _)| {
                                txns.iter()
                                    .find(|t| t["id"].as_str() == Some(k))
                                    .and_then(|t| t["amount"].as_f64())
                                    .unwrap_or(0.0)
                            })
                            .sum()
                    })
                    .unwrap_or(0.0);
                if final_balance.is_nan() {
                    diffs.push(Difference {
                        path: "output.final_balance".into(),
                        expected: json!("number"),
                        actual: e.output.clone(),
                        comparator: "concurrency".into(),
                        message: "missing final_balance".into(),
                    });
                } else {
                    if final_balance < 0.0 {
                        diffs.push(Difference {
                            path: "output.final_balance".into(),
                            expected: json!(">= 0"),
                            actual: json!(final_balance),
                            comparator: "concurrency".into(),
                            message: "negative balance (overdraft)".into(),
                        });
                    }
                    if (initial_balance - committed - final_balance).abs() > 0.5 {
                        diffs.push(Difference {
                            path: "output.final_balance".into(),
                            expected: json!(initial_balance - committed),
                            actual: json!(final_balance),
                            comparator: "concurrency".into(),
                            message:
                                "lost update: balance inconsistent with committed transactions"
                                    .into(),
                        });
                    }
                    if !allowed.is_empty() && !allowed.contains(&(final_balance.round() as i64)) {
                        diffs.push(Difference {
                            path: "output.final_balance".into(),
                            expected: json!(allowed),
                            actual: json!(final_balance),
                            comparator: "concurrency".into(),
                            message: "non-serialisable outcome".into(),
                        });
                    }
                    if let Some(legacy) = &oracle_execs[i] {
                        // Same interleaving on legacy: transaction semantics must match exactly.
                        if legacy.output["final_balance"] != e.output["final_balance"]
                            || legacy.output["outcomes"] != e.output["outcomes"]
                        {
                            diffs.push(Difference {
                                path: "output".into(),
                                expected: legacy.output.clone(),
                                actual: e.output.clone(),
                                comparator: "concurrency-differential".into(),
                                message: "legacy and next disagree on this interleaving".into(),
                            });
                        }
                    }
                }
                json!({ "schedule": schedules[i], "output": e.output })
            }
            Err(err) => {
                diffs.push(Difference {
                    path: "$".into(),
                    expected: json!("execution"),
                    actual: json!(null),
                    comparator: "execution".into(),
                    message: err.to_string(),
                });
                json!({ "schedule": schedules[i] })
            }
        };
        results.push(VerificationResult {
            kind: VerificationKind::Concurrency,
            scenario_id: ScenarioId::new(case.id.clone()),
            function_id: function_id.clone(),
            rule_ids: vec![],
            priority: Priority::P0,
            passed: diffs.is_empty(),
            differences: diffs,
            explained: false,
            explanation: None,
            duration_ms: 0,
            details,
        });
    }
    let passed = results.iter().filter(|r| r.passed).count();
    let schedules_run = results.len();
    Ok((
        results,
        json!({ "schedules": schedules_run, "passed": passed, "allowed_outcomes": allowed }),
    ))
}

fn mk_case(
    id: &str,
    initial_state: &Value,
    txns: &[Value],
    schedule: &[(String, String)],
) -> ReplayCase {
    ReplayCase {
        id: id.to_string(),
        initial_state: initial_state.clone(),
        input: json!({ "transactions": txns }),
        options: ExecOptions {
            schedule: Some(
                json!({ "steps": schedule.iter().map(|(t, op)| json!({ "txn": t, "op": op })).collect::<Vec<_>>() }),
            ),
            ..Default::default()
        },
    }
}

fn permute(v: &mut Vec<usize>, k: usize, f: &mut dyn FnMut(&[usize])) {
    if k == v.len() {
        f(v);
        return;
    }
    for i in k..v.len() {
        v.swap(k, i);
        permute(v, k + 1, f);
        v.swap(k, i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_transactions_six_interleavings() {
        let t = vec![
            (
                "A".to_string(),
                vec!["read".to_string(), "write".to_string()],
            ),
            (
                "B".to_string(),
                vec!["read".to_string(), "write".to_string()],
            ),
        ];
        assert_eq!(interleavings(&t, 100).len(), 6);
    }
}
