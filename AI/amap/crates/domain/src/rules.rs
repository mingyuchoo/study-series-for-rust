use crate::ids::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Where the evidence for a rule came from. Drives the deterministic confidence score.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSources {
    /// Rule was located in source code.
    pub code: bool,
    /// Rule is described in requirement / design documents.
    pub document: bool,
    /// Rule is corroborated by observed production behaviour.
    pub production: bool,
    /// Rule was inferred (no direct code evidence) by an agent.
    pub inferred: bool,
}

impl EvidenceSources {
    /// Baseline confidence table from the design:
    /// 0.99 code+doc+prod, 0.95 code+prod, 0.85 code only, 0.65 inferred, 0.40 unknown.
    pub fn baseline_confidence(&self) -> f64 {
        match (self.code, self.document, self.production, self.inferred) {
            (true, true, true, _) => 0.99,
            (true, _, true, _) => 0.95,
            (true, true, false, _) => 0.90,
            (true, false, false, _) => 0.85,
            (false, _, _, true) => 0.65,
            _ => 0.40,
        }
    }
}

/// A mined business rule (`BR-LOAN-000183`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BusinessRule {
    pub id: RuleId,
    pub function_id: FunctionId,
    pub name: String,
    /// Condition in the invariant DSL, e.g. `customer.grade == "VIP" AND loan.age_years > 3`.
    pub condition: String,
    /// Expected result in the invariant DSL, e.g. `fee == 0`.
    pub result: String,
    pub sources: Vec<SourceLocation>,
    pub db_entities: Vec<DbEntityId>,
    pub interfaces: Vec<InterfaceId>,
    pub observed_production_cases: u64,
    pub evidence: EvidenceSources,
    /// Deterministic confidence (never the LLM's self-reported number).
    pub confidence: f64,
    pub priority: Priority,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A requirement tied to a business function.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    pub id: RequirementId,
    pub function_id: FunctionId,
    pub title: String,
    pub text: String,
    pub confidence: f64,
}

/// A business function / capability (`FN-01882`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BusinessFunction {
    pub id: FunctionId,
    pub name: String,
    pub domain: String,
    pub priority: Priority,
    #[serde(default)]
    pub description: String,
}

/// Supported legacy / next languages for source analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Cobol,
    Java,
    Python,
    Sql,
    Rust,
    Other,
}

impl Language {
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".cbl") || lower.ends_with(".cob") || lower.ends_with(".cpy") {
            Language::Cobol
        } else if lower.ends_with(".java") {
            Language::Java
        } else if lower.ends_with(".py") {
            Language::Python
        } else if lower.ends_with(".sql") {
            Language::Sql
        } else if lower.ends_with(".rs") {
            Language::Rust
        } else {
            Language::Other
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Program,
    Paragraph,
    Section,
    Class,
    Method,
    Function,
    Table,
    Column,
    Interface,
    Batch,
    Copybook,
}

/// Canonical code entity produced by the source analysis engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeEntity {
    pub id: SourceUnitId,
    pub language: Language,
    pub kind: EntityKind,
    pub symbol: String,
    pub location: SourceLocation,
    /// Outgoing call / dependency edges (by entity id).
    pub dependencies: Vec<SourceUnitId>,
    #[serde(default)]
    pub function_id: Option<FunctionId>,
    #[serde(default)]
    pub suspicious: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DbEntity {
    pub id: DbEntityId,
    pub table: String,
    pub column: Option<String>,
    pub data_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterfaceSpec {
    pub id: InterfaceId,
    pub name: String,
    pub kind: String,
    pub direction: String,
}

/// Kinds of edges in the System Knowledge Graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    RequirementToFunction,
    FunctionToRule,
    RuleToSource,
    RuleToDb,
    RuleToInterface,
    RuleToBehavior,
    BehaviorToScenario,
    ScenarioToEvidence,
    SourceCalls,
    SourceReadsDb,
    SourceWritesDb,
    RuleOwnedByService,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub kind: RelationKind,
}

impl Relationship {
    pub fn new(from: impl Into<String>, kind: RelationKind, to: impl Into<String>) -> Self {
        Self { from: from.into(), to: to.into(), kind }
    }
}

/// Architecture decision record produced by the Architecture Agent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureDecision {
    pub id: DecisionId,
    pub function_id: FunctionId,
    pub decision: String,
    pub evidence: Vec<String>,
    pub alternatives: Vec<String>,
    pub risks: Vec<String>,
    pub affected_rules: Vec<RuleId>,
    pub affected_tests: Vec<ScenarioId>,
    /// `BR-1029 → Loan Service` ownership mapping.
    pub rule_ownership: Vec<(RuleId, String)>,
    pub created_at: DateTime<Utc>,
}
