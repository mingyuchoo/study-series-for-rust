use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One LLM call as recorded by the gateway (stack §13, §17 "Immutable Audit Log").
///
/// Entries are appended to the same durable, append-only store as verification evidence so that
/// every artefact an agent produced can be traced back to the prompt version, provider, model and
/// cost that produced it, and so per-run token budgets survive process restarts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LlmAuditEntry {
    /// Immutable ledger identity.
    #[serde(default)]
    pub audit_id: String,
    /// SHA-256 over the recorded fields (excluding this hash and the id).
    #[serde(default)]
    pub content_hash: String,
    pub at: DateTime<Utc>,
    pub run_id: Option<String>,
    pub role: String,
    pub task: String,
    pub prompt_version: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached: bool,
    pub cost_usd: f64,
    pub request_hash: String,
    pub pii_masked: bool,
}

impl LlmAuditEntry {
    /// Tokens that actually consumed budget. Cache hits re-use a recorded answer and cost nothing.
    pub fn billable_tokens(&self) -> u64 {
        if self.cached {
            0
        } else {
            self.input_tokens + self.output_tokens
        }
    }
}
