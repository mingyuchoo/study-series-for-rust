//! RCA Agent (design §18): failure → root-cause hypothesis. It never patches (policy-enforced).
use crate::outputs::{RcaOutput, VerificationOutput};
use crate::workspace::{FileSystemWorkspaceFactory, WorkspaceFactory};
use crate::{context_pack, str_list};
use amap_context::ContextBudget;
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct RcaAgent {
    workspaces: std::sync::Arc<dyn WorkspaceFactory>,
}

impl Default for RcaAgent {
    fn default() -> Self {
        Self {
            workspaces: std::sync::Arc::new(FileSystemWorkspaceFactory),
        }
    }
}

impl RcaAgent {
    pub fn with_workspaces(workspaces: std::sync::Arc<dyn WorkspaceFactory>) -> Self {
        Self { workspaces }
    }
}

#[async_trait]
impl AgentTask for RcaAgent {
    fn name(&self) -> &str {
        "rca"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Rca
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let failures = ctx
            .input_as::<VerificationOutput>("verify")?
            .map(|output| output.failures)
            .unwrap_or_default();
        if failures.is_empty() {
            return AgentResult::typed("no unexplained failures", RcaOutput::default());
        }
        // The RCA agent is denied `patch` by policy — make that explicit and auditable.
        if ctx
            .authorize(
                &Principal::agent("rca"),
                "patch",
                &ctx.function_id.0,
                &ActionContext::default(),
            )
            .is_ok()
        {
            return Err(OrchestrationError::agent(
                "rca",
                "policy misconfiguration: rca must not be allowed to patch",
            ));
        }
        let rule_ids: Vec<String> = failures
            .iter()
            .flat_map(|failure| failure.rule_ids.iter().map(|id| id.0.clone()))
            .collect();
        let mut pack = context_pack(
            ctx,
            "root cause",
            ContextBudget {
                max_tokens: 40_000,
                max_behaviors: 5,
                max_tests: 5,
                max_search_hits: 5,
            },
        )
        .await?;
        pack.rules
            .retain(|r| rule_ids.contains(&r.id.0) || rule_ids.is_empty());
        let workspace = self.workspaces.open(&ctx.config.workspace);
        let next_files = workspace.read_files()?;
        let next_src: String = next_files
            .iter()
            .map(|f| format!("### next/{}\n```\n{}\n```", f.path, f.content))
            .collect::<Vec<_>>()
            .join("\n");
        let failed_text =
            serde_json::to_string_pretty(&failures.iter().take(12).collect::<Vec<_>>()).unwrap();
        let prompt = format!("{}\n\n## Next-system source\n{}\n\n## Failed scenarios (legacy expected vs next actual)\n{}\n", pack.render(), next_src, failed_text);
        let req = LlmRequest::new(TaskKind::RootCauseAnalysis, AgentRole::Rca, "", prompt)
            .with_run(ctx.run_id.0.clone());
        let resp = ctx.llm.complete(req).await?;
        let j: Value = resp
            .json
            .ok_or_else(|| OrchestrationError::agent("rca", "no structured output"))?;
        let affected: Vec<RuleId> = str_list(&j["affected_rules"])
            .into_iter()
            .map(RuleId::new)
            .collect();
        let explained = failures
            .iter()
            .filter(|failure| {
                failure
                    .rule_ids
                    .iter()
                    .any(|rule| affected.iter().any(|candidate| candidate == rule))
                    || affected.is_empty()
            })
            .count();
        let root = RootCause {
            function_id: ctx.function_id.clone(),
            location: j["location"].as_object().map(|l| {
                SourceLocation::new(
                    l["file"].as_str().unwrap_or("?"),
                    l["start_line"].as_u64().unwrap_or(0) as u32,
                    l["end_line"].as_u64().unwrap_or(0) as u32,
                )
            }),
            summary: j["summary"].as_str().unwrap_or("").to_string(),
            legacy_behavior: j["legacy_behavior"].as_str().unwrap_or("").to_string(),
            next_behavior: j["next_behavior"].as_str().unwrap_or("").to_string(),
            affected_rules: affected,
            affected_scenarios: failures.len() as u64,
            confidence: explained as f64 / failures.len() as f64,
        };
        ctx.emit(
            "rca.completed",
            json!({ "summary": root.summary, "confidence": root.confidence }),
        )
        .await;
        AgentResult::typed(
            format!(
                "root cause: {} (confidence {:.1}%)",
                root.summary,
                root.confidence * 100.0
            ),
            RcaOutput {
                root_cause: Some(root),
                provider: Some(resp.provider),
            },
        )
    }
}
