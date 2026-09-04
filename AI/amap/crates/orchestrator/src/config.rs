use amap_domain::QualityGateThresholds;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceSource {
    File {
        path: PathBuf,
    },
    /// Redpanda and Apache Kafka use the same protocol.
    Kafka {
        brokers: String,
        topic: String,
        group_id: String,
        #[serde(default = "default_trace_records")]
        max_records: usize,
        #[serde(default = "default_trace_idle_timeout_ms")]
        idle_timeout_ms: u64,
    },
}

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
    /// Streaming trace sources. The legacy `traces` file remains supported.
    #[serde(default)]
    pub trace_sources: Vec<TraceSource>,
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
    /// Optional absolute latency regression allowance. Unset keeps timing as evidence only.
    #[serde(default)]
    pub timing_tolerance_ms: Option<u64>,
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
    /// gRPC endpoint of a remote verification worker pool; injected from operator settings and
    /// overwritten when a run specification is loaded into the platform.
    #[serde(default)]
    pub verifier_endpoint: Option<String>,
    /// Worker credentials are injected from platform settings and never forwarded in config JSON.
    #[serde(default, skip_serializing)]
    pub verifier_token: Option<String>,
    #[serde(default, skip_serializing)]
    pub verifier_tls_ca: Option<PathBuf>,
    #[serde(default, skip_serializing)]
    pub verifier_tls_cert: Option<PathBuf>,
    #[serde(default, skip_serializing)]
    pub verifier_tls_key: Option<PathBuf>,
    #[serde(default, skip_serializing)]
    pub verifier_tls_domain: Option<String>,
    #[serde(default, skip_serializing)]
    pub verifier_allow_insecure: bool,
    #[serde(default, skip_serializing)]
    pub verifier_artifact_max_bytes: usize,
    /// Maximum LLM tokens per run (0 = unlimited).
    #[serde(default)]
    pub token_budget: u64,
}

fn default_iterations() -> u32 {
    3
}
fn default_trace_records() -> usize {
    1_000_000
}
fn default_trace_idle_timeout_ms() -> u64 {
    30_000
}
fn default_mutants() -> usize {
    40
}
fn default_faults() -> Vec<String> {
    [
        "db_timeout",
        "api_timeout",
        "mq_duplicate",
        "partial_commit",
        "slow_response",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

impl RunConfig {
    pub fn resolved_command(&self, cmd: &[String]) -> Vec<String> {
        cmd.iter()
            .map(|c| {
                c.replace("{workspace}", &self.workspace.display().to_string())
                    .replace("{source_root}", &self.source_root.display().to_string())
            })
            .collect()
    }

    /// Validate every filesystem and process boundary before executing an untrusted run spec.
    pub fn validate_execution_boundary(
        &self,
        root: &Path,
        allowed_executables: &HashSet<String>,
    ) -> Result<(), String> {
        let root = std::fs::canonicalize(root).map_err(|e| format!("worker_root: {e}"))?;
        let ensure_inside = |path: &Path, label: &str, must_exist: bool| -> Result<(), String> {
            if path
                .components()
                .any(|component| component == Component::ParentDir)
            {
                return Err(format!("{label} contains a parent-directory traversal"));
            }
            let candidate = if path.exists() {
                std::fs::canonicalize(path).map_err(|e| format!("{label}: {e}"))?
            } else if must_exist {
                return Err(format!("{label} does not exist: {}", path.display()));
            } else {
                let mut ancestor = path;
                let mut missing = Vec::new();
                while !ancestor.exists() {
                    missing.push(
                        ancestor
                            .file_name()
                            .ok_or_else(|| format!("{label} has no existing ancestor"))?
                            .to_os_string(),
                    );
                    ancestor = ancestor
                        .parent()
                        .ok_or_else(|| format!("{label} has no existing ancestor"))?;
                }
                let mut candidate = std::fs::canonicalize(ancestor)
                    .map_err(|e| format!("{label} ancestor: {e}"))?;
                for component in missing.into_iter().rev() {
                    candidate.push(component);
                }
                candidate
            };
            if !candidate.starts_with(&root) {
                return Err(format!("{label} is outside worker_root"));
            }
            Ok(())
        };

        ensure_inside(&self.workspace, "workspace", false)?;
        ensure_inside(&self.source_root, "source_root", true)?;
        for path in self
            .documents
            .iter()
            .chain(self.comparator_specs.iter())
            .chain(self.traces.iter())
            .chain(self.invariants.iter())
        {
            ensure_inside(path, "support file", true)?;
        }
        for source in &self.trace_sources {
            if let TraceSource::File { path } = source {
                ensure_inside(path, "trace source", true)?;
            }
        }

        for command in std::iter::once(&self.next_command).chain(self.legacy_command.iter()) {
            let resolved = self.resolved_command(command);
            let (program, args) = resolved
                .split_first()
                .ok_or_else(|| "verification command is empty".to_string())?;
            let basename = Path::new(program)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if !allowed_executables.contains(basename) {
                return Err(format!("executable `{basename}` is not allowlisted"));
            }
            if Path::new(program).components().count() > 1 {
                ensure_inside(Path::new(program), "command executable", true)?;
            }
            for argument in args {
                if matches!(argument.as_str(), "-c" | "-e" | "--eval" | "--execute") {
                    return Err("inline code execution arguments are prohibited".into());
                }
                let path = Path::new(argument);
                if path.is_absolute() {
                    ensure_inside(path, "command argument", false)?;
                } else if path
                    .components()
                    .any(|component| component == Component::ParentDir)
                {
                    return Err("command argument contains a parent-directory traversal".into());
                }
            }
        }
        Ok(())
    }
}
