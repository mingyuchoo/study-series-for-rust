//! Fix Agent (design §19): root-cause hypothesis → staged patch (applied only after independent review).
use crate::builder::{files_from_json, write_files};
use crate::rca::workspace_files;
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::json;

#[derive(Default)]
pub struct FixAgent;

pub fn staging_dir(ws: &std::path::Path) -> std::path::PathBuf {
    ws.join(".amap-staging")
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
        let root = ctx.input("rca")["root_cause"].clone();
        if root.is_null() {
            return Ok(AgentResult::new("nothing to fix", json!({ "patch": null })));
        }
        ctx.authorize(&Principal::agent("fix"), "patch", &ctx.function_id.0, &ActionContext::default())?;
        let files = workspace_files(&ctx.config.workspace);
        let src: String = files.iter().map(|f| format!("### {}\n```\n{}\n```", f.path, f.content)).collect::<Vec<_>>().join("\n");
        let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let rule_text: String = rules.iter().map(|r| format!("- {} WHEN {} THEN {}", r.id, r.condition, r.result)).collect::<Vec<_>>().join("\n");
        let prompt = format!("## Root cause hypothesis\n{}\n\n## Business rules\n{}\n\n## Current source\n{}\n", serde_json::to_string_pretty(&root).unwrap(), rule_text, src);
        let req = LlmRequest::new(TaskKind::PatchGeneration, AgentRole::Fix, "", prompt).with_run(ctx.run_id.0.clone());
        let resp = ctx.llm.complete(req).await?;
        let j = resp.json.ok_or_else(|| OrchestrationError::agent("fix", "no structured output"))?;
        let changes = files_from_json(&j["files"]);
        if changes.is_empty() {
            return Err(OrchestrationError::agent("fix", "empty patch"));
        }
        let staging = staging_dir(&ctx.config.workspace);
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(|e| OrchestrationError::Other(e.to_string()))?;
        write_files(&staging, &changes)?;
        let patch = Patch { function_id: ctx.function_id.clone(), rationale: j["rationale"].as_str().unwrap_or("").to_string(), changes: changes.clone(), author_agent: "fix".into() };
        ctx.emit("repair.staged", json!({ "files": changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>() })).await;
        Ok(AgentResult::new(format!("staged patch touching {} file(s): {}", changes.len(), patch.rationale), json!({ "patch": patch, "provider": resp.provider })))
    }
}
