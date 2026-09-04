//! Independent Review Agent (design §20): a different model reviews the staged patch; only an
//! approved patch is applied to the workspace. Policy forbids reviewing one's own change.
use crate::builder::write_files;
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::json;

#[derive(Default)]
pub struct ReviewAgent;

#[async_trait]
impl AgentTask for ReviewAgent {
    fn name(&self) -> &str {
        "review"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Reviewer
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let patch_json = ctx.input("fix")["patch"].clone();
        if patch_json.is_null() {
            return Ok(AgentResult::new(
                "no patch to review",
                json!({ "approved": false, "applied": false }),
            ));
        }
        let patch: Patch = serde_json::from_value(patch_json)
            .map_err(|e| OrchestrationError::agent("review", e))?;
        let author = Principal::agent(&patch.author_agent);
        let action_ctx = ActionContext {
            author: Some(author.clone()),
            ..Default::default()
        };
        // Governance: the author may not approve its own patch; the reviewer may.
        if ctx
            .authorize(&author, "approve", &ctx.function_id.0, &action_ctx)
            .is_ok()
        {
            return Err(OrchestrationError::agent(
                "review",
                "policy misconfiguration: author could self-approve",
            ));
        }
        ctx.authorize(
            &Principal::agent("reviewer"),
            "approve",
            &ctx.function_id.0,
            &action_ctx,
        )?;

        let fix_provider: Option<ModelProvider> =
            serde_json::from_value(ctx.input("fix")["provider"].clone()).ok();
        let root = ctx.input("rca")["root_cause"].clone();
        let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let rule_text: String = rules
            .iter()
            .map(|r| format!("- {} WHEN {} THEN {}", r.id, r.condition, r.result))
            .collect::<Vec<_>>()
            .join("\n");
        let diff: String = patch
            .changes
            .iter()
            .map(|c| format!("### {}\n```\n{}\n```", c.path, c.content))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "## Root cause\n{}\n\n## Rationale\n{}\n\n## Business rules\n{}\n\n## Patch\n{}\n",
            serde_json::to_string_pretty(&root).unwrap(),
            patch.rationale,
            rule_text,
            diff
        );
        let mut req = LlmRequest::new(TaskKind::CodeReview, AgentRole::Reviewer, "", prompt)
            .with_run(ctx.run_id.0.clone());
        if let Some(fp) = fix_provider {
            for candidate in [
                ModelProvider::Anthropic,
                ModelProvider::OpenAi,
                ModelProvider::Local,
                ModelProvider::Mock,
            ] {
                if candidate != fp {
                    req = req.with_provider(candidate);
                    break;
                }
            }
        }
        let resp = match ctx.llm.complete(req.clone()).await {
            Ok(r) => r,
            Err(amap_llm::LlmError::NotConfigured(_)) => {
                ctx.llm
                    .complete(LlmRequest {
                        provider: None,
                        ..req
                    })
                    .await?
            }
            Err(e) => return Err(e.into()),
        };
        let j = resp
            .json
            .unwrap_or(json!({ "approved": false, "comments": ["no structured output"] }));
        let verdict = ReviewVerdict {
            approved: j["approved"].as_bool().unwrap_or(false),
            reviewer_agent: format!("reviewer/{:?}", resp.provider),
            comments: crate::str_list(&j["comments"]),
        };
        let mut applied = false;
        if verdict.approved {
            write_files(&ctx.config.workspace, &patch.changes)?;
            applied = true;
            ctx.emit("repair.applied", json!({ "files": patch.changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>() })).await;
        } else {
            ctx.emit("repair.rejected", json!({ "comments": verdict.comments }))
                .await;
        }
        Ok(AgentResult::new(
            format!(
                "review {} by {} ({} comment(s))",
                if verdict.approved {
                    "APPROVED"
                } else {
                    "REJECTED"
                },
                verdict.reviewer_agent,
                verdict.comments.len()
            ),
            json!({ "approved": verdict.approved, "applied": applied, "verdict": verdict }),
        ))
    }
}
