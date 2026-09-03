//! Uncertainty analysis + risk-based HITL routing (design §21, §24).
use amap_domain::*;
use amap_graph::KnowledgeGraph;
use amap_knowledge::{ReviewRequest, ReviewStatus};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_uncertainty::{assess, ConfidenceVector, Weights};
use async_trait::async_trait;
use serde_json::json;

#[derive(Default)]
pub struct UncertaintyAgent;

pub async fn confidence_vector(ctx: &AgentContext, equivalence: f64) -> Result<ConfidenceVector, OrchestrationError> {
    let reqs = ctx.knowledge.requirements_for(&ctx.function_id).await?;
    let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
    let scenarios = ctx.knowledge.scenarios_for(&ctx.function_id).await?;
    let units = ctx.knowledge.source_units_for(&ctx.function_id).await?;
    let avg = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    let requirement = if reqs.is_empty() { 0.5 } else { avg(&reqs.iter().map(|r| r.confidence).collect::<Vec<_>>()) };
    let rule = avg(&rules.iter().map(|r| r.confidence).collect::<Vec<_>>());
    let behavior = if rules.is_empty() { 0.0 } else { rules.iter().filter(|r| r.observed_production_cases > 0).count() as f64 / rules.len() as f64 };
    let test = if rules.is_empty() { 0.0 } else { (scenarios.len() as f64 / (rules.len() as f64 * 3.0)).min(1.0) };
    let unresolved = units.iter().flat_map(|u| u.dependencies.iter()).filter(|d| d.0.starts_with("external::")).count();
    let dependency = if units.is_empty() { 1.0 } else { 1.0 - (unresolved as f64 / units.len() as f64).min(0.5) };
    let complexity = ((units.len() as f64) / 200.0).min(1.0);
    Ok(ConfidenceVector { requirement, rule, behavior, test, dependency, equivalence, complexity })
}

#[async_trait]
impl AgentTask for UncertaintyAgent {
    fn name(&self) -> &str {
        "uncertainty"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Uncertainty
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let equivalence = ctx.input("verify")["equivalence"].as_f64().unwrap_or(0.0);
        let v = confidence_vector(ctx, equivalence).await?;
        let report = assess(&v, &Weights::default());
        let snap = ctx.knowledge.snapshot().await?;
        let graph = KnowledgeGraph::from_snapshot(&snap);
        let weak = graph.weakly_evidenced_rules();
        ctx.emit("uncertainty.assessed", json!({ "uncertainty": report.uncertainty, "tier": report.tier })).await;

        let mut halt = None;
        if report.tier.requires_human() {
            let reviews = ctx.knowledge.list_reviews().await?;
            let approved = reviews.iter().any(|r| r.function_id == ctx.function_id && r.status == ReviewStatus::Approved);
            if !approved {
                let id = format!("HITL-{}-{}", ctx.function_id.0, ctx.run_id.0);
                let req = ReviewRequest {
                    id: id.clone(),
                    function_id: ctx.function_id.clone(),
                    tier: report.tier,
                    reason: format!("uncertainty {:.1}% — drivers: {:?}", report.uncertainty * 100.0, report.drivers.iter().take(3).collect::<Vec<_>>()),
                    uncertainty: report.uncertainty,
                    status: if ctx.config.auto_approve_hitl { ReviewStatus::Approved } else { ReviewStatus::Pending },
                    decided_by: if ctx.config.auto_approve_hitl { Some("auto-approve (demo)".into()) } else { None },
                };
                ctx.knowledge.queue_review(req).await?;
                ctx.emit("human.review.required", json!({ "review_id": id, "tier": report.tier })).await;
                if !ctx.config.auto_approve_hitl {
                    halt = Some(format!("{:?}: SME decision required (review {id})", report.tier));
                }
            }
        }
        let summary = format!("uncertainty {:.2}% → {:?}; {} weakly-evidenced rules", report.uncertainty * 100.0, report.tier, weak.len());
        let mut res = AgentResult::new(summary, json!({ "report": report, "weakly_evidenced": weak }));
        res.halt = halt;
        Ok(res)
    }
}
