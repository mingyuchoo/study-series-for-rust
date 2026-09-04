//! Bridges the LLM gateway's audit port to the knowledge store, so LLM calls land in the same
//! durable, append-only ledger as verification evidence.
use amap_domain::LlmAuditEntry;
use amap_knowledge::KnowledgeStore;
use amap_llm::AuditSink;
use async_trait::async_trait;
use std::sync::Arc;

pub struct KnowledgeAuditSink {
    knowledge: Arc<dyn KnowledgeStore>,
}

impl KnowledgeAuditSink {
    pub fn new(knowledge: Arc<dyn KnowledgeStore>) -> Self {
        Self { knowledge }
    }
}

#[async_trait]
impl AuditSink for KnowledgeAuditSink {
    async fn record(&self, entry: &LlmAuditEntry) -> Result<(), String> {
        self.knowledge
            .record_llm_audit(entry.clone())
            .await
            .map_err(|e| e.to_string())
    }
    async fn run_usage(&self, run_id: &str) -> Result<u64, String> {
        self.knowledge
            .llm_usage_for_run(run_id)
            .await
            .map_err(|e| e.to_string())
    }
    async fn entries(
        &self,
        run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<LlmAuditEntry>, String> {
        self.knowledge
            .llm_audit(run_id, limit)
            .await
            .map_err(|e| e.to_string())
    }
}
