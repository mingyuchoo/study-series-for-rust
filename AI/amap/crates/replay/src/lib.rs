//! Golden Replay / Differential / State / Property verification engine (design §10–§12).
//!
//! Scenarios are executed against a [`SystemUnderTest`] (the next system, optionally a legacy
//! oracle), compared with deterministic comparators and checked against business invariants.

pub mod sut;

pub use sut::{Execution, ExecOptions, HttpSystem, ProcessSystem, ReplayCase, SystemUnderTest};

use amap_comparator::{ComparatorSpec, ComparisonEngine};
use amap_domain::*;
use amap_invariant::{check_all, InvariantSet};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("system under test error: {0}")]
    Sut(String),
    #[error(transparent)]
    Comparator(#[from] amap_comparator::ComparatorError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub struct ReplayEngine {
    pub comparator: ComparisonEngine,
    pub specs: HashMap<String, ComparatorSpec>,
    pub invariants: InvariantSet,
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayEngine {
    pub fn new() -> Self {
        Self { comparator: ComparisonEngine::new(), specs: HashMap::new(), invariants: InvariantSet::default() }
    }
    pub fn with_spec(mut self, spec: ComparatorSpec) -> Self {
        self.specs.insert(spec.name.clone(), spec);
        self
    }
    pub fn with_invariants(mut self, set: InvariantSet) -> Self {
        self.invariants = set;
        self
    }

    fn spec_for(&self, name: Option<&str>) -> ComparatorSpec {
        name.and_then(|n| self.specs.get(n).cloned()).unwrap_or_else(|| ComparatorSpec::exact("exact"))
    }

    /// Execute scenarios on `next`; when a scenario has no expected output and `oracle` (legacy) is
    /// available, the oracle's output becomes the expectation (differential testing).
    pub async fn replay(
        &self,
        next: &dyn SystemUnderTest,
        oracle: Option<&dyn SystemUnderTest>,
        scenarios: &[TestScenario],
        kind: VerificationKind,
        options: &ExecOptions,
    ) -> Result<Vec<VerificationResult>, ReplayError> {
        let cases: Vec<ReplayCase> = scenarios.iter().map(|s| ReplayCase { id: s.id.0.clone(), initial_state: s.initial_state.clone(), input: s.input.clone(), options: options.clone() }).collect();
        let actuals = next.execute_batch(&cases).await;
        let expected_from_oracle: Vec<Option<Result<Execution, ReplayError>>> = match oracle {
            Some(o) => {
                let need: Vec<usize> = scenarios.iter().enumerate().filter(|(_, s)| s.expected_output.is_none()).map(|(i, _)| i).collect();
                let sub: Vec<ReplayCase> = need.iter().map(|&i| cases[i].clone()).collect();
                let res = if sub.is_empty() { vec![] } else { o.execute_batch(&sub).await };
                let mut out: Vec<Option<Result<Execution, ReplayError>>> = (0..scenarios.len()).map(|_| None).collect();
                for (k, i) in need.into_iter().enumerate() {
                    out[i] = Some(res.get(k).cloned().unwrap_or_else(|| Err(ReplayError::Sut("oracle produced no result".into()))));
                }
                out
            }
            None => (0..scenarios.len()).map(|_| None).collect(),
        };

        let mut results = Vec::with_capacity(scenarios.len());
        for (i, s) in scenarios.iter().enumerate() {
            let spec = self.spec_for(s.comparator_spec.as_deref());
            let mut r = VerificationResult {
                kind,
                scenario_id: s.id.clone(),
                function_id: s.function_id.clone(),
                rule_ids: s.rule_ids.clone(),
                priority: s.priority,
                passed: true,
                differences: vec![],
                explained: false,
                explanation: None,
                duration_ms: 0,
                details: json!({}),
            };
            let actual = match &actuals[i] {
                Ok(a) => a.clone(),
                Err(_) if matches!(&expected_from_oracle[i], Some(Err(_))) => {
                    // Both systems rejected the case at the execution level: equivalent behaviour.
                    r.details = json!({ "both_rejected": true });
                    results.push(r);
                    continue;
                }
                Err(e) => {
                    r.passed = false;
                    r.differences.push(Difference { path: "$".into(), expected: json!("execution"), actual: json!(null), comparator: "execution".into(), message: e.to_string() });
                    results.push(r);
                    continue;
                }
            };
            r.duration_ms = actual.duration_ms;

            let (expected_output, expected_state): (Option<Value>, Option<Value>) = match (&s.expected_output, &expected_from_oracle[i]) {
                (Some(e), _) => (Some(e.clone()), s.expected_state_change.clone()),
                (None, Some(Ok(o))) => (Some(o.output.clone()), Some(o.state_change.clone())),
                (None, Some(Err(e))) => {
                    r.passed = false;
                    r.differences.push(Difference { path: "$".into(), expected: json!("oracle"), actual: json!(null), comparator: "execution".into(), message: e.to_string() });
                    (None, None)
                }
                (None, None) => (None, None),
            };

            if let Some(exp) = &expected_output {
                let cmp = self.comparator.compare(&spec, exp, &actual.output)?;
                if !cmp.equal {
                    r.passed = false;
                    r.differences.extend(cmp.differences.into_iter().map(|mut d| {
                        d.path = format!("output.{}", d.path);
                        d
                    }));
                }
            }
            if let Some(exp) = &expected_state {
                if !exp.is_null() {
                    let cmp = self.comparator.compare(&spec, exp, &actual.state_change)?;
                    if !cmp.equal {
                        r.passed = false;
                        r.differences.extend(cmp.differences.into_iter().map(|mut d| {
                            d.path = format!("state.{}", d.path);
                            d
                        }));
                    }
                }
            }
            if !s.expected_events.is_empty() {
                let cmp = self.comparator.compare(&spec, &Value::Array(s.expected_events.clone()), &Value::Array(actual.events.clone()))?;
                if !cmp.equal {
                    r.passed = false;
                    r.differences.extend(cmp.differences.into_iter().map(|mut d| {
                        d.path = format!("events{}", d.path);
                        d
                    }));
                }
            }

            // Business invariants are checked on the merged view of input, output and state (design §12).
            let merged = merge_view(s, &actual);
            let outcomes = check_all(&self.invariants, &merged);
            let violated: Vec<_> = outcomes.iter().filter(|o| o.applicable && !o.holds).collect();
            if !violated.is_empty() {
                r.passed = false;
                for v in &violated {
                    r.differences.push(Difference { path: format!("invariant.{}", v.name), expected: json!("holds"), actual: json!("violated"), comparator: "invariant".into(), message: v.detail.clone() });
                }
            }
            r.details = json!({
                "actual_output": actual.output,
                "actual_state_change": actual.state_change,
                "expected_output": expected_output,
                "invariants_checked": outcomes.iter().filter(|o| o.applicable).count(),
            });
            results.push(r);
        }
        Ok(results)
    }
}

/// Merged JSON document the invariants see: `{ input, initial_state, output, state_change, events, <output fields flattened> }`.
pub fn merge_view(s: &TestScenario, actual: &Execution) -> Value {
    let mut m = serde_json::Map::new();
    if let Value::Object(o) = &actual.output {
        for (k, v) in o {
            m.insert(k.clone(), v.clone());
        }
    }
    m.insert("input".into(), s.input.clone());
    m.insert("initial_state".into(), s.initial_state.clone());
    m.insert("output".into(), actual.output.clone());
    m.insert("state_change".into(), actual.state_change.clone());
    m.insert("events".into(), Value::Array(actual.events.clone()));
    Value::Object(m)
}

/// Golden Master: turn production behavior records into replayable scenarios.
pub fn scenarios_from_behaviors(behaviors: &[BehaviorRecord], spec_name: Option<&str>, default_priority: Priority) -> Vec<TestScenario> {
    behaviors
        .iter()
        .map(|b| TestScenario {
            id: ScenarioId::new(format!("TEST-{}", b.id.0.trim_start_matches("BH-"))),
            function_id: b.function_id.clone(),
            origin: ScenarioOrigin::GoldenMaster,
            rule_ids: b.related_rules.clone(),
            initial_state: b.initial_state.clone(),
            input: b.input.clone(),
            expected_output: Some(b.legacy_output.clone()),
            expected_state_change: if b.db_state_change.is_null() { None } else { Some(b.db_state_change.clone()) },
            expected_events: b.events.clone(),
            priority: b.priority.unwrap_or(default_priority),
            comparator_spec: spec_name.map(|s| s.to_string()),
            behavior_id: Some(b.id.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct Echo;
    #[async_trait]
    impl SystemUnderTest for Echo {
        fn name(&self) -> &str {
            "echo"
        }
        async fn execute(&self, case: &ReplayCase) -> Result<Execution, ReplayError> {
            Ok(Execution { output: json!({"fee": case.input["fee"], "debit": [{"amount": 1}], "credit": [{"amount": 1}]}), state_change: json!({}), events: vec![], external_calls: vec![], duration_ms: 1 })
        }
    }

    #[tokio::test]
    async fn golden_replay_with_invariants() {
        let inv = amap_invariant::parse("RULE ACC SUM(debit.amount) == SUM(credit.amount)").unwrap();
        let engine = ReplayEngine::new().with_invariants(inv);
        let b = BehaviorRecord {
            id: BehaviorId::new("BH-1"),
            function_id: FunctionId::new("FN-1"),
            initial_state: json!({}),
            input: json!({"fee": 0}),
            legacy_output: json!({"fee": 0, "debit": [{"amount": 1}], "credit": [{"amount": 1}]}),
            db_state_change: Value::Null,
            events: vec![],
            external_calls: vec![],
            timing_ms: None,
            related_rules: vec![],
            priority: Some(Priority::P0),
        };
        let scenarios = scenarios_from_behaviors(&[b], None, Priority::P1);
        let res = engine.replay(&Echo, None, &scenarios, VerificationKind::GoldenReplay, &ExecOptions::default()).await.unwrap();
        assert!(res[0].passed, "{:?}", res[0].differences);
    }
}
