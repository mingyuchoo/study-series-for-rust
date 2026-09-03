//! Builder Agent (design §8): one narrow bounded context → next-system code. Never verifies itself.
use crate::context_pack;
use amap_context::ContextBudget;
use amap_domain::*;
use amap_llm::{Effort, LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::json;
use std::path::{Component, Path, PathBuf};

#[derive(Default)]
pub struct BuilderAgent;

/// Reject path traversal; files are always written under the workspace.
pub fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    if p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir)) {
        return None;
    }
    Some(root.join(p))
}

pub fn write_files(root: &Path, files: &[FileChange]) -> Result<Vec<String>, OrchestrationError> {
    let mut written = Vec::new();
    for f in files {
        let path = safe_join(root, &f.path).ok_or_else(|| OrchestrationError::Other(format!("unsafe path {}", f.path)))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| OrchestrationError::Other(e.to_string()))?;
        }
        std::fs::write(&path, &f.content).map_err(|e| OrchestrationError::Other(e.to_string()))?;
        written.push(f.path.clone());
    }
    Ok(written)
}

pub fn files_from_json(v: &serde_json::Value) -> Vec<FileChange> {
    v.as_array()
        .map(|a| a.iter().filter_map(|f| Some(FileChange { path: f["path"].as_str()?.to_string(), content: f["content"].as_str()?.to_string() })).collect())
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
        ctx.authorize(&Principal::agent("builder"), "build", &ctx.function_id.0, &ActionContext::default())?;
        let pack = context_pack(ctx, "implementation", ContextBudget { max_tokens: 90_000, max_behaviors: 20, max_tests: 20, max_search_hits: 5 }).await?;
        let prompt = format!("{}\n\nImplement this bounded context as the next-generation service. Workspace: {}", pack.render(), ctx.config.workspace.display());
        let req = LlmRequest::new(TaskKind::CodeGeneration, AgentRole::Builder, "", prompt).with_run(ctx.run_id.0.clone()).with_effort(Effort::Xhigh);
        let resp = ctx.llm.complete(req).await?;
        let j = resp.json.ok_or_else(|| OrchestrationError::agent("build", "no structured output"))?;
        let files = files_from_json(&j["files"]);
        if files.is_empty() {
            return Err(OrchestrationError::agent("build", "builder produced no files"));
        }
        std::fs::create_dir_all(&ctx.config.workspace).map_err(|e| OrchestrationError::Other(e.to_string()))?;
        let written = write_files(&ctx.config.workspace, &files)?;
        ctx.emit("build.completed", json!({ "files": written })).await;
        Ok(AgentResult::new(
            format!("built {} file(s) with {:?}/{}", written.len(), resp.provider, resp.model),
            json!({ "files": written, "provider": resp.provider, "model": resp.model, "notes": j["notes"] }),
        ))
    }
}
