use crate::ids::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The independent verification engines of the Verification Factory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationKind {
    Static,
    Unit,
    GoldenReplay,
    Differential,
    State,
    Interface,
    Boundary,
    Property,
    Mutation,
    Adversarial,
    Fault,
    Concurrency,
}

impl VerificationKind {
    pub const ALL: [VerificationKind; 12] = [
        VerificationKind::Static,
        VerificationKind::Unit,
        VerificationKind::GoldenReplay,
        VerificationKind::Differential,
        VerificationKind::State,
        VerificationKind::Interface,
        VerificationKind::Boundary,
        VerificationKind::Property,
        VerificationKind::Mutation,
        VerificationKind::Adversarial,
        VerificationKind::Fault,
        VerificationKind::Concurrency,
    ];
}

/// One concrete difference between expected and actual.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Difference {
    pub path: String,
    pub expected: Value,
    pub actual: Value,
    pub comparator: String,
    pub message: String,
}

/// Outcome of running a single scenario through one verification engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub kind: VerificationKind,
    pub scenario_id: ScenarioId,
    pub function_id: FunctionId,
    pub rule_ids: Vec<RuleId>,
    pub priority: Priority,
    pub passed: bool,
    #[serde(default)]
    pub differences: Vec<Difference>,
    /// A difference that has been analysed and accepted (e.g. new trace id format) is *explained*.
    /// The quality gate demands Unexplained Difference = 0.
    #[serde(default)]
    pub explained: bool,
    #[serde(default)]
    pub explanation: Option<String>,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub details: Value,
}

impl VerificationResult {
    pub fn is_unexplained_failure(&self) -> bool {
        !self.passed && !self.explained
    }
}

/// Root-cause hypothesis produced by the RCA Agent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RootCause {
    pub function_id: FunctionId,
    pub location: Option<SourceLocation>,
    pub summary: String,
    pub legacy_behavior: String,
    pub next_behavior: String,
    pub affected_rules: Vec<RuleId>,
    pub affected_scenarios: u64,
    /// Evidence-derived confidence (fraction of failing scenarios explained by the hypothesis).
    pub confidence: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub content: String,
}

/// A candidate patch produced by the Fix Agent (never applied by the RCA agent itself).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub function_id: FunctionId,
    pub rationale: String,
    pub changes: Vec<FileChange>,
    pub author_agent: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReviewVerdict {
    pub approved: bool,
    pub reviewer_agent: String,
    pub comments: Vec<String>,
}
