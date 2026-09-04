//! System Knowledge Graph — canonical knowledge store (design §6, stack §4).
//!
//! PostgreSQL is the system of record (`PgKnowledgeStore`); an in-memory store backs tests,
//! the CLI demo and unit-level agent runs. Graph traversal is projected in `amap-graph`.

pub mod memory;
#[cfg(feature = "postgres")]
pub mod postgres;

use amap_domain::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub use memory::InMemoryKnowledgeStore;
#[cfg(feature = "postgres")]
pub use postgres::PgKnowledgeStore;

#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type KResult<T> = Result<T, KnowledgeError>;

/// Full snapshot used for graph projection and context assembly.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct KnowledgeSnapshot {
    pub functions: Vec<BusinessFunction>,
    pub requirements: Vec<Requirement>,
    pub rules: Vec<BusinessRule>,
    pub source_units: Vec<CodeEntity>,
    pub db_entities: Vec<DbEntity>,
    pub interfaces: Vec<InterfaceSpec>,
    pub behaviors: Vec<BehaviorRecord>,
    pub scenarios: Vec<TestScenario>,
    pub decisions: Vec<ArchitectureDecision>,
    pub relationships: Vec<Relationship>,
    pub evidence: Vec<EvidenceRecord>,
}

/// Human-in-the-loop review request queued by the orchestrator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub id: String,
    #[serde(default)]
    pub run_id: Option<RunId>,
    pub function_id: FunctionId,
    pub tier: HitlTier,
    pub reason: String,
    pub uncertainty: f64,
    pub status: ReviewStatus,
    #[serde(default = "chrono::Utc::now")]
    pub requested_at: chrono::DateTime<chrono::Utc>,
    /// Verified identity of the decider (OIDC `sub`), or the automation that decided.
    #[serde(default)]
    pub decided_by: Option<String>,
    /// How the decider was authenticated: `oidc:<issuer>`, `auto_approve_hitl`, `insecure-dev-header`.
    #[serde(default)]
    pub decided_via: Option<String>,
    #[serde(default)]
    pub decided_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Durable control-plane state and the latest resumable workflow checkpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: String,
    /// Operator-facing name. Empty only for records created before named runs were introduced.
    #[serde(default)]
    pub display_name: String,
    pub function_id: FunctionId,
    pub status: String,
    pub spec_path: PathBuf,
    /// Immutable, worker-root-relative inputs selected when this run was created.
    #[serde(default)]
    pub inputs: Option<RunInputs>,
    pub mock: bool,
    /// Authenticated subject, service principal, or development actor that created the run.
    #[serde(default)]
    pub started_by: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub outcome: Option<Value>,
    #[serde(default)]
    pub checkpoint: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInputs {
    pub source: LocalPathInput,
    pub destination: LocalPathInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPathInput {
    #[serde(rename = "type")]
    pub kind: String,
    pub path: PathBuf,
}

impl LocalPathInput {
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: "local_path".into(),
            path: path.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Pending,
    Approved,
    Rejected,
}

#[async_trait]
pub trait KnowledgeStore: Send + Sync {
    async fn upsert_function(&self, f: BusinessFunction) -> KResult<()>;
    async fn get_function(&self, id: &FunctionId) -> KResult<Option<BusinessFunction>>;
    async fn list_functions(&self) -> KResult<Vec<BusinessFunction>>;

    async fn upsert_requirement(&self, r: Requirement) -> KResult<()>;
    async fn requirements_for(&self, f: &FunctionId) -> KResult<Vec<Requirement>>;

    async fn upsert_rule(&self, r: BusinessRule) -> KResult<()>;
    async fn rules_for(&self, f: &FunctionId) -> KResult<Vec<BusinessRule>>;

    async fn upsert_source_unit(&self, e: CodeEntity) -> KResult<()>;
    async fn source_units_for(&self, f: &FunctionId) -> KResult<Vec<CodeEntity>>;
    async fn list_source_units(&self) -> KResult<Vec<CodeEntity>>;

    async fn upsert_db_entity(&self, e: DbEntity) -> KResult<()>;
    async fn list_db_entities(&self) -> KResult<Vec<DbEntity>>;

    async fn upsert_interface(&self, i: InterfaceSpec) -> KResult<()>;
    async fn list_interfaces(&self) -> KResult<Vec<InterfaceSpec>>;

    async fn upsert_behavior(&self, b: BehaviorRecord) -> KResult<()>;
    async fn behaviors_for(&self, f: &FunctionId) -> KResult<Vec<BehaviorRecord>>;

    async fn upsert_scenario(&self, s: TestScenario) -> KResult<()>;
    async fn scenarios_for(&self, f: &FunctionId) -> KResult<Vec<TestScenario>>;

    async fn upsert_decision(&self, d: ArchitectureDecision) -> KResult<()>;
    async fn decisions_for(&self, f: &FunctionId) -> KResult<Vec<ArchitectureDecision>>;

    async fn add_relationship(&self, r: Relationship) -> KResult<()>;
    async fn relationships(&self) -> KResult<Vec<Relationship>>;

    async fn record_evidence(&self, e: EvidenceRecord) -> KResult<()>;
    async fn evidence_for(&self, f: &FunctionId) -> KResult<Vec<EvidenceRecord>>;

    async fn queue_review(&self, r: ReviewRequest) -> KResult<()>;
    async fn list_reviews(&self) -> KResult<Vec<ReviewRequest>>;
    async fn decide_review(
        &self,
        id: &str,
        status: ReviewStatus,
        by: &str,
        via: &str,
    ) -> KResult<()>;

    /// Append one LLM call to the immutable audit ledger.
    async fn record_llm_audit(&self, entry: LlmAuditEntry) -> KResult<()>;
    /// Most recent LLM audit entries (newest first), optionally for one run.
    async fn llm_audit(&self, run_id: Option<&str>, limit: usize) -> KResult<Vec<LlmAuditEntry>>;
    /// Billable tokens consumed by a run so far.
    async fn llm_usage_for_run(&self, run_id: &str) -> KResult<u64>;

    async fn upsert_workflow_run(&self, run: WorkflowRun) -> KResult<()>;
    async fn get_workflow_run(&self, id: &str) -> KResult<Option<WorkflowRun>>;
    async fn list_workflow_runs(&self) -> KResult<Vec<WorkflowRun>>;

    async fn snapshot(&self) -> KResult<KnowledgeSnapshot>;
}
