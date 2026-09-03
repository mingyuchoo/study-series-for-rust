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
    pub function_id: FunctionId,
    pub tier: HitlTier,
    pub reason: String,
    pub uncertainty: f64,
    pub status: ReviewStatus,
    #[serde(default)]
    pub decided_by: Option<String>,
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
    async fn decide_review(&self, id: &str, status: ReviewStatus, by: &str) -> KResult<()>;

    async fn snapshot(&self) -> KResult<KnowledgeSnapshot>;
}
