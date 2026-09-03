use amap_domain::QualityGateThresholds;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Per-run configuration (paths, commands, engines). Serialisable so it can travel to workers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig {
    /// Root of the legacy source tree.
    pub source_root: PathBuf,
    /// Requirement / design documents (text or markdown) for rule corroboration.
    #[serde(default)]
    pub documents: Vec<PathBuf>,
    /// Production trace file (JSON lines of behavior records or raw request/response captures).
    #[serde(default)]
    pub traces: Option<PathBuf>,
    /// Directory where the builder writes the next-generation code.
    pub workspace: PathBuf,
    /// Command that runs the next system in JSON-lines mode. `{workspace}` is substituted.
    pub next_command: Vec<String>,
    /// Command that runs the legacy oracle (mainframe replay adapter / emulator), if available.
    #[serde(default)]
    pub legacy_command: Option<Vec<String>>,
    /// Comparator spec YAML files.
    #[serde(default)]
    pub comparator_specs: Vec<PathBuf>,
    /// Default comparator spec name for golden scenarios.
    #[serde(default)]
    pub default_spec: Option<String>,
    /// Business invariant DSL file.
    #[serde(default)]
    pub invariants: Option<PathBuf>,
    #[serde(default = "default_iterations")]
    pub max_fix_iterations: u32,
    #[serde(default = "default_mutants")]
    pub max_mutants: usize,
    #[serde(default = "default_faults")]
    pub faults: Vec<String>,
    /// Concurrency template: `{ "initial_state": {...}, "transactions": [ {...}, {...} ] }`.
    #[serde(default)]
    pub concurrency_template: Option<serde_json::Value>,
    #[serde(default)]
    pub thresholds: QualityGateThresholds,
    /// Auto-approve HITL requests (demo / CI only — never in production).
    #[serde(default)]
    pub auto_approve_hitl: bool,
    /// gRPC endpoint of a remote verification worker pool; in-process when unset.
    #[serde(default)]
    pub verifier_endpoint: Option<String>,
    /// Maximum LLM tokens per run (0 = unlimited).
    #[serde(default)]
    pub token_budget: u64,
}

fn default_iterations() -> u32 {
    3
}
fn default_mutants() -> usize {
    40
}
fn default_faults() -> Vec<String> {
    ["db_timeout", "api_timeout", "mq_duplicate", "partial_commit", "slow_response"].iter().map(|s| s.to_string()).collect()
}

impl RunConfig {
    pub fn resolved_command(&self, cmd: &[String]) -> Vec<String> {
        cmd.iter().map(|c| c.replace("{workspace}", &self.workspace.display().to_string()).replace("{source_root}", &self.source_root.display().to_string())).collect()
    }
}
