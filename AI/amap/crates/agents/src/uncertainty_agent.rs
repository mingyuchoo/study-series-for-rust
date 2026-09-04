//! Uncertainty analysis + risk-based HITL routing (design §21, §24).
use crate::outputs::{UncertaintyOutput, VerificationOutput};
use crate::ports::{UncertaintyDependencies, UncertaintyKnowledge};
use amap_domain::*;
use amap_graph::KnowledgeGraph;
use amap_knowledge::{ReviewRequest, ReviewStatus};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_uncertainty::{
    assess, confidence_vector_from_evidence, ConfidenceEvidence, ConfidenceVector, Weights,
};
use async_trait::async_trait;
use serde_json::json;

#[derive(Default)]
pub struct UncertaintyAgent;

pub async fn confidence_vector<K: UncertaintyKnowledge + ?Sized>(
    dependencies: &UncertaintyDependencies<'_, K>,
    function_id: &FunctionId,
    equivalence: f64,
) -> Result<ConfidenceVector, OrchestrationError> {
    let reqs = dependencies.knowledge.requirements_for(function_id).await?;
    let rules = dependencies.knowledge.rules_for(function_id).await?;
    let scenarios = dependencies.knowledge.scenarios_for(function_id).await?;
    let units = dependencies.knowledge.source_units_for(function_id).await?;
    let unresolved = units
        .iter()
        .flat_map(|u| u.dependencies.iter())
        .filter(|d| d.0.starts_with("external::"))
        .count();
    Ok(confidence_vector_from_evidence(&ConfidenceEvidence {
        requirement_confidences: reqs
            .iter()
            .map(|requirement| requirement.confidence)
            .collect(),
        rule_confidences: rules.iter().map(|rule| rule.confidence).collect(),
        rules_total: rules.len(),
        rules_observed_in_production: rules
            .iter()
            .filter(|rule| rule.observed_production_cases > 0)
            .count(),
        scenarios_total: scenarios.len(),
        source_units_total: units.len(),
        unresolved_dependencies: unresolved,
        equivalence,
    }))
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
        let dependencies = UncertaintyDependencies {
            knowledge: ctx.knowledge.as_ref(),
        };
        let equivalence = ctx
            .input_as::<VerificationOutput>("verify")?
            .map(|output| output.equivalence)
            .unwrap_or(0.0);
        let v = confidence_vector(&dependencies, &ctx.function_id, equivalence).await?;
        let report = assess(&v, &Weights::default());
        let snap = dependencies.knowledge.snapshot().await?;
        let graph = KnowledgeGraph::from_snapshot(&snap);
        let weak = graph.weakly_evidenced_rules();
        ctx.emit(
            "uncertainty.assessed",
            json!({ "uncertainty": report.uncertainty, "tier": report.tier }),
        )
        .await;

        let mut halt = None;
        if report.tier.requires_human() {
            let reviews = dependencies.knowledge.list_reviews().await?;
            let approved = reviews.iter().any(|r| {
                r.run_id.as_ref() == Some(&ctx.run_id)
                    && r.function_id == ctx.function_id
                    && r.status == ReviewStatus::Approved
            });
            if !approved {
                let id = format!("HITL-{}-{}", ctx.function_id.0, ctx.run_id.0);
                let req = ReviewRequest {
                    id: id.clone(),
                    run_id: Some(ctx.run_id.clone()),
                    function_id: ctx.function_id.clone(),
                    tier: report.tier,
                    reason: format!(
                        "uncertainty {:.1}% — drivers: {:?}",
                        report.uncertainty * 100.0,
                        report.drivers.iter().take(3).collect::<Vec<_>>()
                    ),
                    uncertainty: report.uncertainty,
                    status: if ctx.config.auto_approve_hitl {
                        ReviewStatus::Approved
                    } else {
                        ReviewStatus::Pending
                    },
                    requested_at: ctx.clock.now(),
                    decided_by: if ctx.config.auto_approve_hitl {
                        Some("auto-approve (demo)".into())
                    } else {
                        None
                    },
                    decided_at: if ctx.config.auto_approve_hitl {
                        Some(ctx.clock.now())
                    } else {
                        None
                    },
                };
                dependencies.knowledge.queue_review(req).await?;
                ctx.emit(
                    "human.review.required",
                    json!({ "review_id": id, "tier": report.tier }),
                )
                .await;
                if !ctx.config.auto_approve_hitl {
                    halt = Some(format!(
                        "{:?}: SME decision required (review {id})",
                        report.tier
                    ));
                }
            }
        }
        let summary = format!(
            "uncertainty {:.2}% → {:?}; {} weakly-evidenced rules",
            report.uncertainty * 100.0,
            report.tier,
            weak.len()
        );
        let mut res = AgentResult::typed(
            summary,
            UncertaintyOutput {
                report,
                weakly_evidenced: weak
                    .into_iter()
                    .map(|(rule, missing)| (rule, missing.into_iter().map(str::to_owned).collect()))
                    .collect(),
            },
        )?;
        res.halt = halt;
        Ok(res)
    }
}
