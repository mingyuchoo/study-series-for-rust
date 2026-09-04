use crate::ids::*;
use crate::verification::VerificationKind;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single evidence entry. "Done" is not evidence — this is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    /// Immutable ledger identity.
    #[serde(default)]
    pub evidence_id: String,
    /// SHA-256 of the producing verification result.
    #[serde(default)]
    pub content_hash: String,
    pub run_id: RunId,
    pub function_id: FunctionId,
    pub kind: VerificationKind,
    pub scenario_id: String,
    pub rule_ids: Vec<RuleId>,
    pub priority: Priority,
    pub passed: bool,
    pub explained: bool,
    pub producer: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub payload_uri: Option<String>,
    #[serde(default)]
    pub details: Value,
}

/// Per-engine aggregate for a function.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KindSummary {
    pub total: u64,
    pub passed: u64,
    pub unexplained_failures: u64,
}

impl KindSummary {
    pub fn ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.passed as f64 / self.total as f64
        }
    }
}

/// Completion certificate for one business function (design §22).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FunctionCertificate {
    pub function_id: String,
    pub implemented: bool,
    pub requirements_total: u64,
    pub requirements_covered: u64,
    pub rules_total: u64,
    pub rules_covered: u64,
    pub critical_rules_total: u64,
    pub critical_rules_covered: u64,
    pub golden: KindSummary,
    pub boundary: KindSummary,
    pub property: KindSummary,
    pub adversarial: KindSummary,
    pub fault: KindSummary,
    pub concurrency: KindSummary,
    pub production_replay: KindSummary,
    pub mutation_injected: u64,
    pub mutation_detected: u64,
    pub p0_defects_open: u64,
    pub p1_defects_open: u64,
    pub unexplained_differences: u64,
    pub residual_uncertainty: f64,
}

impl FunctionCertificate {
    pub fn mutation_score(&self) -> f64 {
        if self.mutation_injected == 0 {
            0.0
        } else {
            self.mutation_detected as f64 / self.mutation_injected as f64
        }
    }
}
