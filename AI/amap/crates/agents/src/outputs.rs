//! Typed contracts exchanged by modernization workflow steps.
//!
//! Serialization belongs to the generic DAG checkpoint boundary. Business workflow code uses these
//! types instead of indexing unvalidated JSON with string keys.

use amap_assurance::VerificationFailure;
use amap_domain::*;
use amap_uncertainty::{ResidualUncertainty, UncertaintyReport};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryOutput {
    pub entities: usize,
    pub dependency_edges: usize,
    pub files: usize,
    pub suspicious: Vec<String>,
    pub domains: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleMiningOutput {
    pub rules: Vec<String>,
    pub unparsable: usize,
    pub provider: String,
    pub n_version: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BehaviorMiningOutput {
    pub behaviors: usize,
    pub links: usize,
    pub malformed: usize,
    pub contradicted: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyOutput {
    pub report: UncertaintyReport,
    pub weakly_evidenced: Vec<(String, Vec<String>)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureOutput {
    pub decision_id: DecisionId,
    pub ownership: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildOutput {
    pub files: Vec<String>,
    pub provider: ModelProvider,
    pub model: String,
    pub notes: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioGenerationOutput {
    pub scenarios: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ModelProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_probes: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerificationOutput {
    pub certified: bool,
    pub gate: GateReport,
    pub certificate: FunctionCertificate,
    pub equivalence: f64,
    pub equivalence_metrics: EquivalenceMetrics,
    pub residual: ResidualUncertainty,
    pub failures: Vec<VerificationFailure>,
    pub metrics: Map<String, Value>,
    pub oracle_derived_expectations: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RcaOutput {
    pub root_cause: Option<RootCause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ModelProvider>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FixOutput {
    pub patch: Option<Patch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ModelProvider>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReviewOutput {
    pub approved: bool,
    pub applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<ReviewVerdict>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkflowState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<DiscoveryOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_mining: Option<RuleMiningOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior_mining: Option<BehaviorMiningOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<UncertaintyOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<ArchitectureOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_generation: Option<ScenarioGenerationOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary: Option<ScenarioGenerationOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adversarial: Option<ScenarioGenerationOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<VerificationOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rca: Option<RcaOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewOutput>,
}

impl WorkflowState {
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        if value.is_null() {
            Ok(Self::default())
        } else {
            serde_json::from_value(value)
        }
    }

    pub fn to_value(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    pub fn merge_value(&mut self, value: Value) -> Result<(), serde_json::Error> {
        let newer = Self::from_value(value)?;
        macro_rules! replace_present {
            ($($field:ident),+ $(,)?) => {
                $(if newer.$field.is_some() { self.$field = newer.$field; })+
            };
        }
        replace_present!(
            discovery,
            rule_mining,
            behavior_mining,
            uncertainty,
            architecture,
            build,
            test_generation,
            boundary,
            adversarial,
            verify,
            rca,
            fix,
            review,
        );
        Ok(())
    }
}
