//! RCA Agent (design §18): failure → root-cause hypothesis. It never patches (policy-enforced).
use crate::{context_pack, str_list};
use amap_context::ContextBudget;
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::{json, Value};
use walkdir::WalkDir;

#[derive(Default)]
pub struct RcaAgent;

pub fn workspace_files(ws: &std::path::Path) -> Vec<FileChange> {
    WalkDir::new(ws)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(ws).ok()?.display().to_string();
            if rel.starts_with(".amap") {
                return None;
            }
            Some(FileChange {
                path: rel,
                content: std::fs::read_to_string(e.path()).ok()?,
            })
        })
        .collect()
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
        let verify = ctx.input("verify");
        let failures = verify["failures"].as_array().cloned().unwrap_or_default();
        if failures.is_empty() {
            return Ok(AgentResult::new(
                "no unexplained failures",
                json!({ "root_cause": null }),
            ));
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
            .flat_map(|f| str_list(&f["rules"]))
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
        let next_files = workspace_files(&ctx.config.workspace);
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
            .filter(|f| {
                str_list(&f["rules"])
                    .iter()
                    .any(|r| affected.iter().any(|a| a.0 == *r))
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
        Ok(AgentResult::new(
            format!(
                "root cause: {} (confidence {:.1}%)",
                root.summary,
                root.confidence * 100.0
            ),
            json!({ "root_cause": root, "provider": resp.provider }),
        ))
    }
}
