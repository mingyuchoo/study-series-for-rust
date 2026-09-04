//! Builder Agent (design §8): one narrow bounded context → next-system code. Never verifies itself.
use crate::context_pack;
use crate::outputs::BuildOutput;
use crate::workspace::{FileSystemWorkspaceFactory, WorkspaceFactory};
use amap_context::ContextBudget;
use amap_domain::*;
use amap_llm::{Effort, LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::json;

pub struct BuilderAgent {
    workspaces: std::sync::Arc<dyn WorkspaceFactory>,
}

impl Default for BuilderAgent {
    fn default() -> Self {
        Self {
            workspaces: std::sync::Arc::new(FileSystemWorkspaceFactory),
        }
    }
}

impl BuilderAgent {
    pub fn with_workspaces(workspaces: std::sync::Arc<dyn WorkspaceFactory>) -> Self {
        Self { workspaces }
    }
}

pub fn files_from_json(v: &serde_json::Value) -> Vec<FileChange> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    Some(FileChange {
                        path: f["path"].as_str()?.to_string(),
                        content: f["content"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl AgentTask for BuilderAgent {
    fn name(&self) -> &str {
        "build"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Builder
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        ctx.authorize(
            &Principal::agent("builder"),
            "build",
            &ctx.function_id.0,
            &ActionContext::default(),
        )?;
        let pack = context_pack(
            ctx,
            "implementation",
            ContextBudget {
                max_tokens: 90_000,
                max_behaviors: 20,
                max_tests: 20,
                max_search_hits: 5,
            },
        )
        .await?;
        let prompt = format!(
            "{}\n\nImplement this bounded context as the next-generation service. Workspace: {}",
            pack.render(),
            ctx.config.workspace.display()
        );
        let req = LlmRequest::new(TaskKind::CodeGeneration, AgentRole::Builder, "", prompt)
            .with_run(ctx.run_id.0.clone())
            .with_effort(Effort::Xhigh);
        let resp = ctx.llm.complete(req).await?;
        let j = resp
            .json
            .ok_or_else(|| OrchestrationError::agent("build", "no structured output"))?;
        let files = files_from_json(&j["files"]);
        if files.is_empty() {
            return Err(OrchestrationError::agent(
                "build",
                "builder produced no files",
            ));
        }
        let workspace = self.workspaces.open(&ctx.config.workspace);
        let written = workspace.apply(&files)?;
        ctx.emit("build.completed", json!({ "files": written }))
            .await;
        AgentResult::typed(
            format!(
                "built {} file(s) with {:?}/{}",
                written.len(),
                resp.provider,
                resp.model
            ),
            BuildOutput {
                files: written,
                provider: resp.provider,
                model: resp.model,
                notes: j["notes"].clone(),
            },
        )
    }
}
