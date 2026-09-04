//! Fix Agent (design §19): root-cause hypothesis → staged patch (applied only after independent review).
use crate::builder::files_from_json;
use crate::outputs::{FixOutput, RcaOutput};
use crate::workspace::{FileSystemWorkspaceFactory, WorkspaceFactory};
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::json;

pub struct FixAgent {
    workspaces: std::sync::Arc<dyn WorkspaceFactory>,
}

impl Default for FixAgent {
    fn default() -> Self {
        Self {
            workspaces: std::sync::Arc::new(FileSystemWorkspaceFactory),
        }
    }
}

impl FixAgent {
    pub fn with_workspaces(workspaces: std::sync::Arc<dyn WorkspaceFactory>) -> Self {
        Self { workspaces }
    }
}

#[async_trait]
impl AgentTask for FixAgent {
    fn name(&self) -> &str {
        "fix"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Fix
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let Some(root) = ctx
            .input_as::<RcaOutput>("rca")?
            .and_then(|output| output.root_cause)
        else {
            return AgentResult::typed("nothing to fix", FixOutput::default());
        };
        ctx.authorize(
            &Principal::agent("fix"),
            "patch",
            &ctx.function_id.0,
            &ActionContext::default(),
        )?;
        let workspace = self.workspaces.open(&ctx.config.workspace);
        let files = workspace.read_files()?;
        let src: String = files
            .iter()
            .map(|f| format!("### {}\n```\n{}\n```", f.path, f.content))
            .collect::<Vec<_>>()
            .join("\n");
        let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let rule_text: String = rules
            .iter()
            .map(|r| format!("- {} WHEN {} THEN {}", r.id, r.condition, r.result))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "## Root cause hypothesis\n{}\n\n## Business rules\n{}\n\n## Current source\n{}\n",
            serde_json::to_string_pretty(&root).unwrap(),
            rule_text,
            src
        );
        let req = LlmRequest::new(TaskKind::PatchGeneration, AgentRole::Fix, "", prompt)
            .with_run(ctx.run_id.0.clone());
        let resp = ctx.llm.complete(req).await?;
        let j = resp
            .json
            .ok_or_else(|| OrchestrationError::agent("fix", "no structured output"))?;
        let changes = files_from_json(&j["files"]);
        if changes.is_empty() {
            return Err(OrchestrationError::agent("fix", "empty patch"));
        }
        workspace.stage(&changes)?;
        let patch = Patch {
            function_id: ctx.function_id.clone(),
            rationale: j["rationale"].as_str().unwrap_or("").to_string(),
            changes: changes.clone(),
            author_agent: "fix".into(),
        };
        ctx.emit(
            "repair.staged",
            json!({ "files": changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>() }),
        )
        .await;
        AgentResult::typed(
            format!(
                "staged patch touching {} file(s): {}",
                changes.len(),
                patch.rationale
            ),
            FixOutput {
                patch: Some(patch),
                provider: Some(resp.provider),
            },
        )
    }
}
