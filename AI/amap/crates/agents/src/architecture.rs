//! Architecture Agent (design §7): maps legacy function → target architecture; writes no code.
use crate::outputs::ArchitectureOutput;
use crate::{context_pack, str_list};
use amap_context::ContextBudget;
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use async_trait::async_trait;

#[derive(Default)]
pub struct ArchitectureAgent;

#[async_trait]
impl AgentTask for ArchitectureAgent {
    fn name(&self) -> &str {
        "architecture"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Architecture
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let pack = context_pack(
            ctx,
            "architecture domain service ownership",
            ContextBudget::default(),
        )
        .await?;
        let req = LlmRequest::new(
            TaskKind::ArchitectureMapping,
            AgentRole::Architecture,
            "",
            pack.render(),
        )
        .with_run(ctx.run_id.0.clone());
        let resp = ctx.llm.complete(req).await?;
        let j = resp
            .json
            .ok_or_else(|| OrchestrationError::agent("architecture", "no structured output"))?;
        let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let known = |id: &str| rules.iter().any(|r| r.id.0 == id);
        let affected_rules: Vec<RuleId> = str_list(&j["affected_rules"])
            .into_iter()
            .filter(|r| known(r))
            .map(RuleId::new)
            .collect();
        let rule_ownership: Vec<(RuleId, String)> = j["rule_ownership"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|o| {
                        Some((
                            RuleId::new(o["rule_id"].as_str()?),
                            o["service"].as_str()?.to_string(),
                        ))
                    })
                    .filter(|(r, _)| known(&r.0))
                    .collect()
            })
            .unwrap_or_default();
        let decision = ArchitectureDecision {
            id: DecisionId::new(format!("ADR-{}-{}", ctx.function_id.0, ctx.run_id.0)),
            function_id: ctx.function_id.clone(),
            decision: j["decision"].as_str().unwrap_or("").to_string(),
            evidence: str_list(&j["evidence"]),
            alternatives: str_list(&j["alternatives"]),
            risks: str_list(&j["risks"]),
            affected_rules,
            affected_tests: str_list(&j["affected_tests"])
                .into_iter()
                .map(ScenarioId::new)
                .collect(),
            rule_ownership: rule_ownership.clone(),
            created_at: ctx.clock.now(),
        };
        if decision.evidence.is_empty() || decision.risks.is_empty() {
            return Err(OrchestrationError::agent(
                "architecture",
                "decision must carry evidence and risks",
            ));
        }
        for (rule, service) in &rule_ownership {
            ctx.knowledge
                .add_relationship(Relationship::new(
                    rule.0.clone(),
                    RelationKind::RuleOwnedByService,
                    service.clone(),
                ))
                .await?;
        }
        ctx.knowledge.upsert_decision(decision.clone()).await?;
        AgentResult::typed(
            format!("architecture decision recorded: {}", decision.decision),
            ArchitectureOutput {
                decision_id: decision.id,
                ownership: rule_ownership.len(),
            },
        )
    }
}
