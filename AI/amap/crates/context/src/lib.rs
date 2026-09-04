//! Context Engine (stack §14): builds a bounded, PII-filtered [`ContextPack`] for an agent task from
//! the knowledge graph (exact), Tantivy (lexical) and a vector index (semantic) — never the whole repo.

pub mod pack;
pub mod search;
pub mod vector;

pub use pack::{CodeSnippet, ContextBudget, ContextPack};
pub use search::{Hit, LexicalIndex};
pub use vector::{InMemoryVectorIndex, OpenAiEmbeddingProvider, VectorIndex};

use amap_domain::*;
use amap_knowledge::KnowledgeStore;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("index error: {0}")]
    Index(String),
    #[error(transparent)]
    Knowledge(#[from] amap_knowledge::KnowledgeError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub struct ContextEngine {
    lexical: LexicalIndex,
    vector: InMemoryVectorIndex,
    embedding_provider: Option<OpenAiEmbeddingProvider>,
    pending_embeddings: Vec<(String, String, String)>,
    source_root: Option<PathBuf>,
}

impl ContextEngine {
    pub fn new(source_root: Option<&Path>) -> Result<Self, ContextError> {
        Ok(Self {
            lexical: LexicalIndex::in_memory()?,
            vector: InMemoryVectorIndex::default(),
            embedding_provider: OpenAiEmbeddingProvider::from_env(),
            pending_embeddings: vec![],
            source_root: source_root.map(|p| p.to_path_buf()),
        })
    }

    /// Index everything retrievable: requirements, rules, source units (with text when available), behaviors.
    pub async fn index(&mut self, store: &dyn KnowledgeStore) -> Result<usize, ContextError> {
        let snap = store.snapshot().await?;
        let mut n = 0;
        for r in &snap.requirements {
            self.add(&r.id.0, "requirement", &format!("{} {}", r.title, r.text))?;
            n += 1;
        }
        for r in &snap.rules {
            self.add(
                &r.id.0,
                "rule",
                &format!("{} {} {}", r.name, r.condition, r.result),
            )?;
            n += 1;
        }
        for e in &snap.source_units {
            let body = self.read_snippet(&e.location).unwrap_or_default();
            self.add(
                &e.id.0,
                "source",
                &format!("{} {} {}", e.symbol, e.location, body),
            )?;
            n += 1;
        }
        for b in &snap.behaviors {
            self.add(
                &b.id.0,
                "behavior",
                &format!("{} {}", b.input, b.legacy_output),
            )?;
            n += 1;
        }
        if let Some(provider) = self.embedding_provider.clone() {
            let pending = std::mem::take(&mut self.pending_embeddings);
            for batch in pending.chunks(64) {
                let input: Vec<String> = batch.iter().map(|(_, _, text)| text.clone()).collect();
                let embeddings = provider.embed(&input).await.map_err(ContextError::Index)?;
                for ((id, kind, _), embedding) in batch.iter().zip(embeddings) {
                    self.vector.add_embedding(id, kind, embedding);
                }
            }
        } else {
            self.pending_embeddings.clear();
        }
        self.lexical.commit()?;
        Ok(n)
    }

    fn add(&mut self, id: &str, kind: &str, text: &str) -> Result<(), ContextError> {
        let masked = amap_llm::pii::mask(text);
        self.lexical.add(id, kind, &masked)?;
        self.vector.add(id, kind, &masked);
        self.pending_embeddings
            .push((id.to_string(), kind.to_string(), masked));
        Ok(())
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<Hit>, ContextError> {
        let mut hits = self.lexical.search(query, limit).unwrap_or_default();
        let semantic = if let Some(provider) = &self.embedding_provider {
            let query = vec![amap_llm::pii::mask(query)];
            let embedding = provider
                .embed(&query)
                .await
                .map_err(ContextError::Index)?
                .into_iter()
                .next()
                .unwrap_or_default();
            self.vector.search_embedding(&embedding, limit)
        } else {
            self.vector.search(query, limit)
        };
        for h in semantic {
            if !hits.iter().any(|x| x.id == h.id) {
                hits.push(h);
            }
        }
        hits.truncate(limit);
        Ok(hits)
    }

    pub fn read_snippet(&self, loc: &SourceLocation) -> Option<String> {
        let root = self.source_root.as_ref()?;
        let text = std::fs::read_to_string(root.join(&loc.file)).ok()?;
        let lines: Vec<&str> = text.lines().collect();
        let start = loc.start_line.max(1) as usize - 1;
        let end = (loc.end_line as usize).min(lines.len());
        if start >= end {
            return None;
        }
        Some(
            lines[start..end]
                .iter()
                .enumerate()
                .map(|(i, l)| format!("{:>5} | {}", start + i + 1, l))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    /// Assemble the bounded context for a function (design §8: narrow bounded context per agent).
    pub async fn build(
        &self,
        store: &dyn KnowledgeStore,
        function_id: &FunctionId,
        task_hint: &str,
        budget: ContextBudget,
    ) -> Result<ContextPack, ContextError> {
        let function = store.get_function(function_id).await?;
        let requirements = store.requirements_for(function_id).await?;
        let rules = store.rules_for(function_id).await?;
        let mut source = Vec::new();
        for unit in store.source_units_for(function_id).await? {
            let text = self
                .read_snippet(&unit.location)
                .unwrap_or_else(|| format!("<{} {}>", unit.symbol, unit.location));
            source.push(CodeSnippet {
                location: unit.location.clone(),
                symbol: unit.symbol.clone(),
                text: amap_llm::pii::mask(&text),
            });
        }
        for r in &rules {
            for loc in &r.sources {
                if source.iter().any(|s| s.location == *loc) {
                    continue;
                }
                if let Some(text) = self.read_snippet(loc) {
                    source.push(CodeSnippet {
                        location: loc.clone(),
                        symbol: r.name.clone(),
                        text: amap_llm::pii::mask(&text),
                    });
                }
            }
        }
        let schema = store
            .list_db_entities()
            .await?
            .into_iter()
            .filter(|d| {
                rules.iter().any(|r| {
                    r.db_entities
                        .iter()
                        .any(|x| x.0 == d.id.0 || x.0 == d.table)
                })
            })
            .collect();
        let interfaces = store
            .list_interfaces()
            .await?
            .into_iter()
            .filter(|i| rules.iter().any(|r| r.interfaces.contains(&i.id)))
            .collect();
        let mut behaviors = store.behaviors_for(function_id).await?;
        behaviors.truncate(budget.max_behaviors);
        let mut tests = store.scenarios_for(function_id).await?;
        tests.truncate(budget.max_tests);
        let decisions = store.decisions_for(function_id).await?;
        let related = self.search(task_hint, budget.max_search_hits).await?;
        let mut pack = ContextPack {
            function,
            requirements,
            rules,
            source,
            schema,
            interfaces,
            behaviors,
            tests,
            decisions,
            related,
            truncated: false,
            estimated_tokens: 0,
        };
        pack.fit(budget.max_tokens);
        Ok(pack)
    }
}
