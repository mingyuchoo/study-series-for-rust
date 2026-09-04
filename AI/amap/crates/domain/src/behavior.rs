use crate::ids::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A production behavior record — the strongest form of evidence and the future Golden Master.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BehaviorRecord {
    pub id: BehaviorId,
    pub function_id: FunctionId,
    #[serde(default)]
    pub initial_state: Value,
    pub input: Value,
    pub legacy_output: Value,
    #[serde(default)]
    pub db_state_change: Value,
    #[serde(default)]
    pub events: Vec<Value>,
    #[serde(default)]
    pub external_calls: Vec<Value>,
    #[serde(default)]
    pub timing_ms: Option<u64>,
    #[serde(default)]
    pub related_rules: Vec<RuleId>,
    #[serde(default)]
    pub priority: Option<Priority>,
}

/// Test scenario derived from behaviors (golden), boundary generation, adversarial probing, …
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestScenario {
    pub id: ScenarioId,
    pub function_id: FunctionId,
    pub origin: ScenarioOrigin,
    pub rule_ids: Vec<RuleId>,
    #[serde(default)]
    pub initial_state: Value,
    pub input: Value,
    /// Expected output; `None` for scenarios where only invariants can be checked.
    pub expected_output: Option<Value>,
    #[serde(default)]
    pub expected_state_change: Option<Value>,
    /// `None` means no event assertion; `Some([])` asserts that no events are emitted.
    #[serde(default)]
    pub expected_events: Option<Vec<Value>>,
    /// `None` means no outbound-call assertion; `Some([])` asserts that no calls are made.
    #[serde(default)]
    pub expected_external_calls: Option<Vec<Value>>,
    /// Captured reference latency. Enforced only when the replay engine has a timing tolerance.
    #[serde(default)]
    pub expected_timing_ms: Option<u64>,
    #[serde(default)]
    pub priority: Priority,
    /// Name of the comparator spec (data-driven equivalence policy).
    #[serde(default)]
    pub comparator_spec: Option<String>,
    #[serde(default)]
    pub behavior_id: Option<BehaviorId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioOrigin {
    GoldenMaster,
    Boundary,
    Property,
    Adversarial,
    Fault,
    Concurrency,
    Manual,
}
