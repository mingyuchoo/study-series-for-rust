//! The LLM Gateway core: PII filtering, caching, retries, token budgets, audit and cost accounting.
//!
//! Audit entries are the system of record for what each agent asked and what it cost. When an
//! [`AuditSink`] is attached the gateway writes every call there *before* handing the answer to
//! the caller and *before* caching it: a call whose audit record cannot be persisted fails
//! closed. Per-run token budgets are enforced against the sink, so they survive restarts and are
//! shared between replicas that use the same store.
use crate::{pii, prompts, LlmClient, LlmError, LlmRequest, LlmResponse, Router};
pub use amap_domain::LlmAuditEntry as AuditEntry;
use amap_domain::ModelProvider;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Entries retained in process memory as a hot view (the sink is the durable record).
const HOT_AUDIT_ENTRIES: usize = 10_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub cache_enabled: bool,
    pub max_retries: u32,
    /// Per-run token budget (input + output of non-cached calls). 0 = unlimited.
    pub run_token_budget: u64,
    pub pii_filter: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            cache_enabled: true,
            max_retries: 3,
            run_token_budget: 0,
            pii_filter: true,
        }
    }
}

/// Durable, append-only destination for audit entries and the source of truth for run usage.
#[async_trait]
pub trait AuditSink: Send + Sync {
    async fn record(&self, entry: &AuditEntry) -> Result<(), String>;
    /// Billable tokens already consumed by a run.
    async fn run_usage(&self, run_id: &str) -> Result<u64, String>;
    /// Most recent entries, optionally filtered by run.
    async fn entries(&self, run_id: Option<&str>, limit: usize) -> Result<Vec<AuditEntry>, String>;
}

pub struct Gateway {
    router: Router,
    config: GatewayConfig,
    sink: Option<Arc<dyn AuditSink>>,
    cache: Mutex<HashMap<String, LlmResponse>>,
    audit: Mutex<VecDeque<AuditEntry>>,
    run_usage: Mutex<HashMap<String, u64>>,
}

/// Rough list prices ($/1M tokens) for cost accounting.
fn price(provider: ModelProvider, model: &str) -> (f64, f64) {
    match provider {
        ModelProvider::Anthropic => {
            if model.contains("fable") {
                (10.0, 50.0)
            } else if model.contains("opus") {
                (5.0, 25.0)
            } else if model.contains("sonnet") {
                (2.0, 10.0)
            } else {
                (1.0, 5.0)
            }
        }
        ModelProvider::OpenAi | ModelProvider::Bedrock => (5.0, 25.0),
        ModelProvider::Azure => {
            // Azure bills the underlying OpenAI model; size tiers are cheaper.
            if model.contains("nano") {
                (0.1, 0.4)
            } else if model.contains("mini") {
                (0.5, 2.0)
            } else {
                (5.0, 25.0)
            }
        }
        ModelProvider::Local | ModelProvider::Mock => (0.0, 0.0),
    }
}

/// Content hash over every recorded field except the id and the hash itself.
pub fn content_hash(entry: &AuditEntry) -> String {
    let mut h = Sha256::new();
    h.update(format!(
        "{}|{:?}|{}|{}|{}|{}|{}|{}|{}|{}|{:.10}|{}|{}",
        entry.at.to_rfc3339(),
        entry.run_id,
        entry.role,
        entry.task,
        entry.prompt_version,
        entry.provider,
        entry.model,
        entry.input_tokens,
        entry.output_tokens,
        entry.cached,
        entry.cost_usd,
        entry.request_hash,
        entry.pii_masked
    ));
    hex::encode(h.finalize())
}

impl Gateway {
    pub fn new(router: Router, config: GatewayConfig) -> Self {
        Self {
            router,
            config,
            sink: None,
            cache: Mutex::new(HashMap::new()),
            audit: Mutex::new(VecDeque::new()),
            run_usage: Mutex::new(HashMap::new()),
        }
    }

    /// Persist every audit entry to `sink` and enforce budgets against it.
    pub fn with_audit_sink(mut self, sink: Arc<dyn AuditSink>) -> Self {
        self.sink = Some(sink);
        self
    }

    pub fn router(&self) -> &Router {
        &self.router
    }

    pub fn has_audit_sink(&self) -> bool {
        self.sink.is_some()
    }

    /// In-process hot view of recent entries (bounded). Use [`Gateway::audit_entries`] for the
    /// durable record.
    pub fn audit_log(&self) -> Vec<AuditEntry> {
        self.audit.lock().unwrap().iter().cloned().collect()
    }

    /// Durable audit entries from the sink, falling back to the hot view without one.
    pub async fn audit_entries(
        &self,
        run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AuditEntry>, LlmError> {
        match &self.sink {
            Some(sink) => sink.entries(run_id, limit).await.map_err(LlmError::Audit),
            None => Ok(self
                .audit_log()
                .into_iter()
                .filter(|e| run_id.is_none_or(|r| e.run_id.as_deref() == Some(r)))
                .rev()
                .take(limit)
                .collect()),
        }
    }

    /// Billable tokens consumed by a run (in-process view).
    pub fn run_usage(&self, run_id: &str) -> u64 {
        self.run_usage
            .lock()
            .unwrap()
            .get(run_id)
            .copied()
            .unwrap_or(0)
    }

    async fn durable_run_usage(&self, run_id: &str) -> Result<u64, LlmError> {
        match &self.sink {
            Some(sink) => sink.run_usage(run_id).await.map_err(LlmError::Audit),
            None => Ok(self.run_usage(run_id)),
        }
    }

    fn hash(req: &LlmRequest, provider: ModelProvider) -> String {
        let mut h = Sha256::new();
        h.update(format!(
            "{:?}|{:?}|{}|{}|{:?}|{:?}",
            req.task, provider, req.system, req.prompt, req.schema, req.effort
        ));
        hex::encode(h.finalize())
    }

    /// N-version reasoning: ask every configured provider independently (design §27).
    pub async fn n_version(
        &self,
        req: LlmRequest,
    ) -> Vec<(ModelProvider, Result<LlmResponse, LlmError>)> {
        let mut out = Vec::new();
        for p in self.router.configured() {
            if p == ModelProvider::Mock && self.router.configured().len() > 1 {
                continue;
            }
            let r = self.complete(req.clone().with_provider(p)).await;
            out.push((p, r));
        }
        out
    }
}

#[async_trait]
impl LlmClient for Gateway {
    async fn complete(&self, mut req: LlmRequest) -> Result<LlmResponse, LlmError> {
        if req.system.is_empty() {
            req.system = prompts::system_prompt(req.task);
        }
        if req.schema.is_none() {
            req.schema = Some(prompts::template(req.task).schema);
        }
        let mut pii_masked = false;
        if self.config.pii_filter {
            let masked = pii::mask(&req.prompt);
            pii_masked = masked != req.prompt;
            req.prompt = masked;
        }
        let provider = self.router.resolve(&req)?;
        let key = Self::hash(&req, provider);

        if let Some(run) = &req.run_id {
            if self.config.run_token_budget > 0
                && self.durable_run_usage(run).await? >= self.config.run_token_budget
            {
                return Err(LlmError::BudgetExceeded(run.clone()));
            }
        }

        if self.config.cache_enabled {
            let hit = self.cache.lock().unwrap().get(&key).cloned();
            if let Some(mut hit) = hit {
                hit.cached = true;
                self.record(&req, &hit, &key, pii_masked).await?;
                return Ok(hit);
            }
        }

        let client = self
            .router
            .provider(provider)
            .ok_or(LlmError::NotConfigured(provider))?;
        let mut attempt = 0;
        let resp = loop {
            match client.complete(req.clone()).await {
                Ok(r) => break r,
                Err(LlmError::Transient(msg)) if attempt < self.config.max_retries => {
                    attempt += 1;
                    let backoff = Duration::from_millis(500 * 2u64.pow(attempt));
                    tracing::warn!(attempt, %msg, "transient LLM error; retrying in {backoff:?}");
                    tokio::time::sleep(backoff).await;
                }
                Err(e) => return Err(e),
            }
        };
        // Audit first: an answer that cannot be accounted for is not handed out or cached.
        self.record(&req, &resp, &key, pii_masked).await?;
        if self.config.cache_enabled {
            self.cache.lock().unwrap().insert(key, resp.clone());
        }
        Ok(resp)
    }
}

impl Gateway {
    async fn record(
        &self,
        req: &LlmRequest,
        resp: &LlmResponse,
        key: &str,
        pii_masked: bool,
    ) -> Result<(), LlmError> {
        let (pi, po) = price(resp.provider, &resp.model);
        let cost = if resp.cached {
            0.0
        } else {
            (resp.input_tokens as f64 * pi + resp.output_tokens as f64 * po) / 1_000_000.0
        };
        let mut entry = AuditEntry {
            audit_id: format!("LLM-{}", uuid::Uuid::new_v4()),
            content_hash: String::new(),
            at: Utc::now(),
            run_id: req.run_id.clone(),
            role: req.role.as_str().to_string(),
            task: format!("{:?}", req.task),
            prompt_version: prompts::template(req.task).version.to_string(),
            provider: format!("{:?}", resp.provider),
            model: resp.model.clone(),
            input_tokens: resp.input_tokens,
            output_tokens: resp.output_tokens,
            cached: resp.cached,
            cost_usd: cost,
            request_hash: key.to_string(),
            pii_masked,
        };
        entry.content_hash = content_hash(&entry);
        if let Some(sink) = &self.sink {
            sink.record(&entry).await.map_err(|error| {
                tracing::error!(%error, audit_id = %entry.audit_id, "LLM audit persistence failed; refusing the response");
                LlmError::Audit(error)
            })?;
        }
        if let Some(run) = &req.run_id {
            *self
                .run_usage
                .lock()
                .unwrap()
                .entry(run.clone())
                .or_insert(0) += entry.billable_tokens();
        }
        tracing::info!(role = %entry.role, task = %entry.task, provider = %entry.provider, model = %entry.model, cached = entry.cached, cost_usd = entry.cost_usd, audit_id = %entry.audit_id, "llm call");
        let mut hot = self.audit.lock().unwrap();
        if hot.len() >= HOT_AUDIT_ENTRIES {
            hot.pop_front();
        }
        hot.push_back(entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MockProvider, RouterConfig, TaskKind};
    use amap_domain::AgentRole;
    use std::sync::Arc;

    fn gateway(config: GatewayConfig) -> (Gateway, Arc<MockProvider>) {
        let mock = Arc::new(MockProvider::new());
        let router =
            Router::new(RouterConfig::default()).with_provider(ModelProvider::Mock, mock.clone());
        (Gateway::new(router, config), mock)
    }

    fn request(prompt: &str) -> LlmRequest {
        LlmRequest::new(TaskKind::CodeReview, AgentRole::Reviewer, "", prompt).with_run("RUN-1")
    }

    #[tokio::test]
    async fn caches_and_masks() {
        let (gw, mock) = gateway(GatewayConfig::default());
        let req = request("customer 900101-1234567 review this");
        let a = gw.complete(req.clone()).await.unwrap();
        let b = gw.complete(req).await.unwrap();
        assert!(!a.cached && b.cached);
        assert_eq!(mock.calls().len(), 1);
        assert!(mock.calls()[0].prompt.contains("[RRN]"));
        let log = gw.audit_log();
        assert_eq!(log.len(), 2);
        assert!(log.iter().all(|e| e.audit_id.starts_with("LLM-")));
        assert!(log.iter().all(|e| e.content_hash == content_hash(e)));
        // Cache hits do not consume budget.
        assert_eq!(
            gw.run_usage("RUN-1"),
            log[0].input_tokens + log[0].output_tokens
        );
    }

    /// Sink that stores entries and can be told to fail, to prove fail-closed behaviour.
    #[derive(Default)]
    struct MemorySink {
        entries: Mutex<Vec<AuditEntry>>,
        fail: Mutex<bool>,
    }

    #[async_trait]
    impl AuditSink for MemorySink {
        async fn record(&self, entry: &AuditEntry) -> Result<(), String> {
            if *self.fail.lock().unwrap() {
                return Err("disk full".into());
            }
            self.entries.lock().unwrap().push(entry.clone());
            Ok(())
        }
        async fn run_usage(&self, run_id: &str) -> Result<u64, String> {
            Ok(self
                .entries
                .lock()
                .unwrap()
                .iter()
                .filter(|e| e.run_id.as_deref() == Some(run_id))
                .map(AuditEntry::billable_tokens)
                .sum())
        }
        async fn entries(
            &self,
            run_id: Option<&str>,
            limit: usize,
        ) -> Result<Vec<AuditEntry>, String> {
            Ok(self
                .entries
                .lock()
                .unwrap()
                .iter()
                .filter(|e| run_id.is_none_or(|r| e.run_id.as_deref() == Some(r)))
                .rev()
                .take(limit)
                .cloned()
                .collect())
        }
    }

    #[tokio::test]
    async fn audit_sink_is_written_before_the_answer_and_fails_closed() {
        let sink = Arc::new(MemorySink::default());
        let (gw, mock) = gateway(GatewayConfig::default());
        let gw = gw.with_audit_sink(sink.clone());
        gw.complete(request("first")).await.unwrap();
        assert_eq!(sink.entries.lock().unwrap().len(), 1);
        assert_eq!(gw.audit_entries(Some("RUN-1"), 10).await.unwrap().len(), 1);

        *sink.fail.lock().unwrap() = true;
        let err = gw.complete(request("second")).await.unwrap_err();
        assert!(matches!(err, LlmError::Audit(_)), "{err}");
        // The unaudited answer was neither cached nor added to the hot view.
        assert_eq!(gw.audit_log().len(), 1);
        *sink.fail.lock().unwrap() = false;
        gw.complete(request("second")).await.unwrap();
        assert_eq!(
            mock.calls().len(),
            3,
            "the failed call was not served from cache"
        );
    }

    #[tokio::test]
    async fn budget_is_enforced_from_the_durable_sink() {
        let sink = Arc::new(MemorySink::default());
        // Pre-existing usage from a previous process for the same run.
        sink.entries.lock().unwrap().push(AuditEntry {
            audit_id: "LLM-old".into(),
            content_hash: String::new(),
            at: Utc::now(),
            run_id: Some("RUN-1".into()),
            role: "builder".into(),
            task: "CodeGeneration".into(),
            prompt_version: "x".into(),
            provider: "Mock".into(),
            model: "mock".into(),
            input_tokens: 90,
            output_tokens: 20,
            cached: false,
            cost_usd: 0.0,
            request_hash: "h".into(),
            pii_masked: false,
        });
        let (gw, _) = gateway(GatewayConfig {
            run_token_budget: 100,
            ..GatewayConfig::default()
        });
        let gw = gw.with_audit_sink(sink);
        let err = gw.complete(request("over budget")).await.unwrap_err();
        assert!(matches!(err, LlmError::BudgetExceeded(_)), "{err}");
    }
}
