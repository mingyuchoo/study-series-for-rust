//! Discovery Agent (design §3): deterministic source analysis + LLM domain classification.
pub mod cobol;
pub mod structured;
pub mod treesitter;

use crate::outputs::DiscoveryOutput;
use crate::{context_pack, str_list};
use amap_context::ContextBudget;
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashSet;
use walkdir::WalkDir;

#[derive(Default)]
pub struct DiscoveryAgent;

pub struct AnalyzedFile {
    pub entities: Vec<CodeEntity>,
    pub db_entities: Vec<DbEntity>,
    pub relationships: Vec<Relationship>,
    pub interfaces: Vec<InterfaceSpec>,
}

/// Run the language-specific analyzers over a source tree.
pub fn analyze_tree(root: &std::path::Path) -> Vec<(String, AnalyzedFile)> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .display()
            .to_string();
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let analyzed = match Language::from_path(&rel) {
            Language::Cobol => Some(cobol::analyze(&rel, &text)),
            Language::Java => treesitter::analyze(&rel, &text, Language::Java),
            Language::Python => treesitter::analyze(&rel, &text, Language::Python),
            Language::Rust => treesitter::analyze(&rel, &text, Language::Rust),
            Language::Javascript => treesitter::analyze(&rel, &text, Language::Javascript),
            Language::Csharp => treesitter::analyze(&rel, &text, Language::Csharp),
            Language::Sql => Some(structured::analyze_sql(&rel, &text)),
            Language::Jcl => Some(structured::analyze_jcl(&rel, &text)),
            _ => None,
        };
        if let Some(a) = analyzed {
            out.push((rel, a));
        }
    }
    out
}

#[async_trait]
impl AgentTask for DiscoveryAgent {
    fn name(&self) -> &str {
        "discovery"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Discovery
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let analyzed = analyze_tree(&ctx.config.source_root);
        let mut entity_count = 0;
        let mut suspicious = Vec::new();
        let mut all_entities = Vec::new();
        for (_file, a) in &analyzed {
            for mut e in a.entities.clone() {
                e.function_id = Some(ctx.function_id.clone());
                if e.suspicious {
                    suspicious.push(e.id.0.clone());
                }
                all_entities.push(e.clone());
                ctx.knowledge.upsert_source_unit(e).await?;
                entity_count += 1;
            }
            for d in a.db_entities.clone() {
                ctx.knowledge.upsert_db_entity(d).await?;
            }
            for i in a.interfaces.clone() {
                ctx.knowledge.upsert_interface(i).await?;
            }
            for r in a.relationships.clone() {
                ctx.knowledge.add_relationship(r).await?;
            }
        }

        // Dead / suspicious code: paragraphs or methods nobody calls (excluding entry points).
        let called: HashSet<String> = all_entities
            .iter()
            .flat_map(|e| e.dependencies.iter().map(|d| d.0.clone()))
            .collect();
        for e in &all_entities {
            let is_entry = matches!(e.kind, EntityKind::Program | EntityKind::Class)
                || e.symbol.to_uppercase().contains("MAIN");
            if !is_entry && !called.contains(&e.id.0) && !suspicious.contains(&e.id.0) {
                suspicious.push(e.id.0.clone());
                let mut flagged = e.clone();
                flagged.suspicious = true;
                ctx.knowledge.upsert_source_unit(flagged).await?;
            }
        }

        // LLM: domain classification of the discovered entities (AI + deterministic tooling).
        let pack = context_pack(
            ctx,
            "domain classification",
            ContextBudget {
                max_tokens: 30_000,
                ..Default::default()
            },
        )
        .await?;
        let entity_list: Vec<String> = all_entities
            .iter()
            .map(|e| format!("{} ({:?}, {})", e.id, e.kind, e.location))
            .collect();
        let prompt = format!(
            "{}\n\n## Entities\n{}\n",
            pack.render(),
            entity_list.join("\n")
        );
        let req = LlmRequest::new(
            TaskKind::DomainClassification,
            AgentRole::Discovery,
            "",
            prompt,
        )
        .with_run(ctx.run_id.0.clone());
        let mut domains = json!([]);
        match ctx.llm.complete(req).await {
            Ok(resp) => {
                if let Some(j) = resp.json {
                    domains = j.get("domains").cloned().unwrap_or(json!([]));
                    for s in str_list(&j["suspicious"]) {
                        if !suspicious.contains(&s) {
                            suspicious.push(s);
                        }
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "domain classification skipped"),
        }

        let dependency_edges: usize = all_entities.iter().map(|e| e.dependencies.len()).sum();
        ctx.emit(
            "discovery.completed",
            json!({ "entities": entity_count, "suspicious": suspicious.len() }),
        )
        .await;
        AgentResult::typed(
            format!("discovered {entity_count} code entities, {dependency_edges} dependency edges, {} suspicious", suspicious.len()),
            DiscoveryOutput {
                entities: entity_count,
                dependency_edges,
                files: analyzed.len(),
                suspicious,
                domains,
            },
        )
    }
}
