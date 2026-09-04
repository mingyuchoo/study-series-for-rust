//! Independent Test Agent: generates scenarios from rules/requirements *without* seeing the implementation.
use crate::outputs::ScenarioGenerationOutput;
use crate::{context_pack, str_list};
use amap_context::ContextBudget;
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::json;

#[derive(Default)]
pub struct TestGeneratorAgent;

#[async_trait]
impl AgentTask for TestGeneratorAgent {
    fn name(&self) -> &str {
        "test_generation"
    }
    fn role(&self) -> AgentRole {
        AgentRole::TestGenerator
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        ctx.authorize(
            &Principal::agent("test_generator"),
            "write_verification_test",
            &ctx.function_id.0,
            &ActionContext::default(),
        )?;
        let mut pack = context_pack(ctx, "test scenarios", ContextBudget::default()).await?;
        pack.source.clear(); // rules + requirements only: independence from any implementation
        let req = LlmRequest::new(
            TaskKind::TestGeneration,
            AgentRole::TestGenerator,
            "",
            pack.render(),
        )
        .with_run(ctx.run_id.0.clone());
        let resp = ctx.llm.complete(req).await?;
        let j = resp.json.unwrap_or(json!({"scenarios": []}));
        let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let mut count = 0;
        for (i, s) in j["scenarios"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let rule_ids: Vec<RuleId> = str_list(&s["rule_ids"])
                .into_iter()
                .filter(|r| rules.iter().any(|x| x.id.0 == *r))
                .map(RuleId::new)
                .collect();
            let priority = rule_ids
                .iter()
                .filter_map(|r| rules.iter().find(|x| x.id == *r))
                .map(|x| x.priority)
                .min()
                .unwrap_or(Priority::P2);
            let scenario = TestScenario {
                id: ScenarioId::new(format!("TEST-GEN-{:04}", i + 1)),
                function_id: ctx.function_id.clone(),
                origin: ScenarioOrigin::Property,
                rule_ids,
                initial_state: s["initial_state"].clone(),
                input: s["input"].clone(),
                expected_output: s.get("expected_output").filter(|v| !v.is_null()).cloned(),
                expected_state_change: None,
                expected_events: None,
                expected_external_calls: None,
                expected_timing_ms: None,
                priority,
                comparator_spec: ctx.config.default_spec.clone(),
                behavior_id: None,
            };
            if scenario.input.is_null() {
                continue;
            }
            ctx.knowledge.upsert_scenario(scenario).await?;
            count += 1;
        }
        AgentResult::typed(
            format!("generated {count} independent scenarios"),
            ScenarioGenerationOutput {
                scenarios: count,
                provider: Some(resp.provider),
                llm_probes: None,
            },
        )
    }
}
