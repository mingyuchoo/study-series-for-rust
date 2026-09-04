//! Business Rule Mining Agent (design §4): LLM extracts rules; the platform validates them
//! (DSL parse) and assigns confidence deterministically from evidence sources.
use crate::{context_pack, priority_from, str_list};
use amap_context::ContextBudget;
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_uncertainty::{n_version_agreement, rule_confidence, EvidenceSignals};
use async_trait::async_trait;
use serde_json::{json, Value};

#[derive(Default)]
pub struct RuleMinerAgent;

fn load_documents(paths: &[std::path::PathBuf]) -> String {
    paths
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|t| amap_llm::pii::mask(&t))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[async_trait]
impl AgentTask for RuleMinerAgent {
    fn name(&self) -> &str {
        "rule_mining"
    }
    fn role(&self) -> AgentRole {
        AgentRole::RuleMiner
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let pack = context_pack(
            ctx,
            "business rules conditions fees interest",
            ContextBudget::default(),
        )
        .await?;
        let docs = load_documents(&ctx.config.documents);
        let prompt = format!(
            "{}\n\n## Documents\n{}\n\nExtract all business rules.",
            pack.render(),
            if docs.is_empty() {
                "(none)".to_string()
            } else {
                docs.clone()
            }
        );
        let req = LlmRequest::new(
            TaskKind::BusinessRuleExtraction,
            AgentRole::RuleMiner,
            "",
            prompt,
        )
        .with_run(ctx.run_id.0.clone());
        let resp = ctx.llm.complete(req.clone()).await?;
        let extracted = resp.json.clone().unwrap_or(json!({"rules": []}));
        let rules_json = extracted["rules"].as_array().cloned().unwrap_or_default();

        // N-version reasoning for P0/P1 rules when a second provider is available (design §27).
        let second_opinion = if ctx.config.token_budget != u64::MAX {
            match ctx
                .llm
                .complete(req.with_provider(ModelProvider::Anthropic))
                .await
            {
                Ok(r) if r.provider != resp.provider => r.json,
                _ => None,
            }
        } else {
            None
        };

        let existing = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let mut stored = 0;
        let mut unparsable = 0;
        let mut ids = Vec::new();
        for (i, r) in rules_json.iter().enumerate() {
            let name = r["name"].as_str().unwrap_or("unnamed").to_string();
            let condition = r["condition"].as_str().unwrap_or("true").to_string();
            let result = r["result"].as_str().unwrap_or("true").to_string();
            let parses = amap_invariant::parse_expr(&condition).is_ok()
                && amap_invariant::parse_expr(&result).is_ok();
            if !parses {
                unparsable += 1;
            }
            let src = &r["source"];
            let location = SourceLocation::new(
                src["file"].as_str().unwrap_or("unknown"),
                src["start_line"].as_u64().unwrap_or(0) as u32,
                src["end_line"].as_u64().unwrap_or(0) as u32,
            );
            let code_evidence = ctx.config.source_root.join(&location.file).exists();
            let in_document = r["in_document"].as_bool().unwrap_or(false)
                || (!docs.is_empty() && docs.to_lowercase().contains(&name.to_lowercase()));
            let evidence = EvidenceSources {
                code: code_evidence,
                document: in_document,
                production: false,
                inferred: !code_evidence,
            };
            let mut signals = EvidenceSignals::default();
            if let Some(other) = &second_opinion {
                let mine = format!("{condition} => {result}");
                let theirs: Vec<String> = other["rules"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|x| {
                                format!(
                                    "{} => {}",
                                    x["condition"].as_str().unwrap_or(""),
                                    x["result"].as_str().unwrap_or("")
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let (agree, _) = n_version_agreement(&[
                    mine.clone(),
                    theirs
                        .iter()
                        .find(|t| **t == mine)
                        .cloned()
                        .unwrap_or_default(),
                ]);
                if agree {
                    signals.two_models_agree = true;
                } else {
                    signals.models_disagree = true;
                }
            }
            let mut confidence = rule_confidence(&evidence, &signals);
            if !parses {
                confidence = confidence.min(0.40); // "의미 불명"
            }
            let id = existing
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.id.clone())
                .unwrap_or_else(|| {
                    RuleId::new(format!(
                        "BR-{}-{:06}",
                        ctx.function_id.0.trim_start_matches("FN-").to_uppercase(),
                        existing.len() + i + 1
                    ))
                });
            let valid_requirements = ctx.knowledge.requirements_for(&ctx.function_id).await?;
            let requirement_ids: Vec<RequirementId> = str_list(&r["requirement_ids"])
                .into_iter()
                .filter(|candidate| valid_requirements.iter().any(|req| req.id.0 == *candidate))
                .map(RequirementId::new)
                .collect();
            let rule = BusinessRule {
                id: id.clone(),
                function_id: ctx.function_id.clone(),
                name,
                condition,
                result,
                sources: vec![location.clone()],
                db_entities: str_list(&r["db_entities"])
                    .into_iter()
                    .map(DbEntityId::new)
                    .collect(),
                interfaces: str_list(&r["interfaces"])
                    .into_iter()
                    .map(InterfaceId::new)
                    .collect(),
                requirement_ids: requirement_ids.clone(),
                observed_production_cases: 0,
                evidence,
                confidence,
                priority: priority_from(&r["priority"]),
                tags: if parses {
                    vec![]
                } else {
                    vec!["unparsable".into()]
                },
            };
            ctx.knowledge
                .add_relationship(Relationship::new(
                    ctx.function_id.0.clone(),
                    RelationKind::FunctionToRule,
                    id.0.clone(),
                ))
                .await?;
            for requirement_id in requirement_ids {
                ctx.knowledge
                    .add_relationship(Relationship::new(
                        requirement_id.0,
                        RelationKind::RequirementToRule,
                        id.0.clone(),
                    ))
                    .await?;
            }
            ctx.knowledge.upsert_rule(rule).await?;
            ids.push(id.0);
            stored += 1;
        }
        ctx.emit(
            "rules.mined",
            json!({ "count": stored, "unparsable": unparsable }),
        )
        .await;
        Ok(AgentResult::new(
            format!("mined {stored} business rules ({unparsable} unparsable → confidence capped at 0.40)"),
            json!({ "rules": ids, "unparsable": unparsable, "provider": format!("{:?}", resp.provider), "n_version": second_opinion.is_some() }),
        ))
    }
}

pub fn rule_as_invariant(rule: &BusinessRule) -> Option<amap_invariant::Invariant> {
    let guard = amap_invariant::parse_expr(&rule.condition).ok()?;
    let body = amap_invariant::parse_expr(&rule.result).ok()?;
    Some(amap_invariant::Invariant {
        name: rule.id.0.clone(),
        guard: Some(guard),
        body,
    })
}

pub fn rule_matches(rule: &BusinessRule, view: &Value) -> Option<bool> {
    let guard = amap_invariant::parse_expr(&rule.condition).ok()?;
    amap_invariant::eval_bool(&guard, view).ok()
}
