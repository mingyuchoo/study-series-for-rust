//! The AMAP agent pool. Builder and Verifier are structurally separate agents with separate
//! contexts; final PASS/FAIL is always computed by deterministic engines.

pub mod adversarial;
pub mod architecture;
pub mod behavior_miner;
pub mod boundary;
pub mod builder;
pub mod discovery;
pub mod fix;
pub mod rca;
pub mod review;
pub mod rule_miner;
pub mod test_generator;
pub mod uncertainty_agent;
pub mod verification;
pub mod verifier;
pub mod workflow;

pub use adversarial::AdversarialAgent;
pub use architecture::ArchitectureAgent;
pub use behavior_miner::BehaviorMinerAgent;
pub use boundary::BoundaryAgent;
pub use builder::BuilderAgent;
pub use discovery::DiscoveryAgent;
pub use fix::FixAgent;
pub use rca::RcaAgent;
pub use review::ReviewAgent;
pub use rule_miner::RuleMinerAgent;
pub use test_generator::TestGeneratorAgent;
pub use uncertainty_agent::UncertaintyAgent;
pub use verifier::VerifierAgent;
pub use workflow::{run_modernization, WorkflowOutcome};

use amap_context::{ContextBudget, ContextEngine, ContextPack};
use amap_domain::*;
use amap_orchestrator::{AgentContext, OrchestrationError};
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Build the bounded context pack for the current function.
pub async fn context_pack(
    ctx: &AgentContext,
    task_hint: &str,
    budget: ContextBudget,
) -> Result<ContextPack, OrchestrationError> {
    let mut engine = ContextEngine::new(Some(&ctx.config.source_root))
        .map_err(|e| OrchestrationError::Other(e.to_string()))?;
    engine
        .index(ctx.knowledge.as_ref())
        .await
        .map_err(|e| OrchestrationError::Other(e.to_string()))?;
    engine
        .build(ctx.knowledge.as_ref(), &ctx.function_id, task_hint, budget)
        .await
        .map_err(|e| OrchestrationError::Other(e.to_string()))
}

/// Convert verification results into evidence-ledger records.
pub fn evidence_from_results(
    ctx: &AgentContext,
    results: &[VerificationResult],
    producer: &str,
) -> Vec<EvidenceRecord> {
    results
        .iter()
        .map(|r| {
            let encoded = serde_json::to_vec(r).unwrap_or_default();
            let content_hash = hex::encode(Sha256::digest(encoded));
            EvidenceRecord {
                evidence_id: format!("EV-{}", uuid::Uuid::new_v4()),
                content_hash,
                run_id: ctx.run_id.clone(),
                function_id: r.function_id.clone(),
                kind: r.kind,
                scenario_id: r.scenario_id.0.clone(),
                rule_ids: r.rule_ids.clone(),
                priority: r.priority,
                passed: r.passed,
                explained: r.explained,
                producer: producer.to_string(),
                created_at: Utc::now(),
                payload_uri: None,
                details: serde_json::json!({ "differences": r.differences, "details": r.details }),
            }
        })
        .collect()
}

pub(crate) fn str_list(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn priority_from(v: &Value) -> Priority {
    match v.as_str().unwrap_or("P2") {
        "P0" => Priority::P0,
        "P1" => Priority::P1,
        "P3" => Priority::P3,
        _ => Priority::P2,
    }
}
