//! Lexical retrieval with Tantivy.
use crate::ContextError;
use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexWriter, TantivyDocument};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub id: String,
    pub kind: String,
    pub score: f32,
}

pub struct LexicalIndex {
    index: Index,
    writer: IndexWriter,
    id: Field,
    kind: Field,
    body: Field,
}

impl LexicalIndex {
    pub fn in_memory() -> Result<Self, ContextError> {
        let mut sb = Schema::builder();
        let id = sb.add_text_field("id", STRING | STORED);
        let kind = sb.add_text_field("kind", STRING | STORED);
        let body = sb.add_text_field("body", TEXT);
        let index = Index::create_in_ram(sb.build());
        let writer = index
            .writer(30_000_000)
            .map_err(|e| ContextError::Index(e.to_string()))?;
        Ok(Self {
            index,
            writer,
            id,
            kind,
            body,
        })
    }

    pub fn add(&mut self, id: &str, kind: &str, text: &str) -> Result<(), ContextError> {
        self.writer
            .add_document(doc!(self.id => id, self.kind => kind, self.body => text))
            .map_err(|e| ContextError::Index(e.to_string()))?;
        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), ContextError> {
        self.writer
            .commit()
            .map_err(|e| ContextError::Index(e.to_string()))?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Hit>, ContextError> {
        let reader = self
            .index
            .reader()
            .map_err(|e| ContextError::Index(e.to_string()))?;
        let searcher = reader.searcher();
        let parser = QueryParser::for_index(&self.index, vec![self.body]);
        let sanitized: String = query
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() || c == '_' {
                    c
                } else {
                    ' '
                }
            })
            .collect();
        if sanitized.trim().is_empty() {
            return Ok(vec![]);
        }
        let q = parser.parse_query_lenient(&sanitized).0;
        let top = searcher
            .search(&q, &TopDocs::with_limit(limit.max(1)).order_by_score())
            .map_err(|e| ContextError::Index(e.to_string()))?;
        let mut hits = Vec::new();
        for (score, addr) in top {
            let d: TantivyDocument = searcher
                .doc(addr)
                .map_err(|e| ContextError::Index(e.to_string()))?;
            let get = |f: Field| {
                d.get_first(f)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            hits.push(Hit {
                id: get(self.id),
                kind: get(self.kind),
                score,
            });
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexes_and_searches() {
        let mut ix = LexicalIndex::in_memory().unwrap();
        ix.add("BR-1", "rule", "VIP customer early repayment fee waiver")
            .unwrap();
        ix.add("BR-2", "rule", "interest accrual daily 365")
            .unwrap();
        ix.commit().unwrap();
        let hits = ix.search("early repayment fee", 5).unwrap();
        assert_eq!(hits[0].id, "BR-1");
    }
}
