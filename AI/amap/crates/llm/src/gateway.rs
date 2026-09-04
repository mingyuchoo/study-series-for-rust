//! The LLM Gateway core: PII filtering, caching, retries, token budgets, audit and cost accounting.
use crate::{pii, prompts, LlmClient, LlmError, LlmRequest, LlmResponse, Router};
use amap_domain::ModelProvider;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub cache_enabled: bool,
    pub max_retries: u32,
    /// Per-run token budget (input + output). 0 = unlimited.
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub at: DateTime<Utc>,
    pub run_id: Option<String>,
    pub role: String,
    pub task: String,
    pub prompt_version: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached: bool,
    pub cost_usd: f64,
    pub request_hash: String,
    pub pii_masked: bool,
}

pub struct Gateway {
    router: Router,
    config: GatewayConfig,
    cache: Mutex<HashMap<String, LlmResponse>>,
    audit: Mutex<Vec<AuditEntry>>,
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
        ModelProvider::Local | ModelProvider::Mock => (0.0, 0.0),
    }
}

impl Gateway {
    pub fn new(router: Router, config: GatewayConfig) -> Self {
        Self {
            router,
            config,
            cache: Mutex::new(HashMap::new()),
            audit: Mutex::new(Vec::new()),
            run_usage: Mutex::new(HashMap::new()),
        }
    }

    pub fn router(&self) -> &Router {
        &self.router
    }

    pub fn audit_log(&self) -> Vec<AuditEntry> {
        self.audit.lock().unwrap().clone()
    }

    pub fn run_usage(&self, run_id: &str) -> u64 {
        self.run_usage
            .lock()
            .unwrap()
            .get(run_id)
            .copied()
            .unwrap_or(0)
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
                && self.run_usage(run) >= self.config.run_token_budget
            {
                return Err(LlmError::BudgetExceeded(run.clone()));
            }
        }

        if self.config.cache_enabled {
            if let Some(hit) = self.cache.lock().unwrap().get(&key).cloned() {
                let mut hit = hit;
                hit.cached = true;
                self.record(&req, &hit, &key, pii_masked);
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
        if self.config.cache_enabled {
            self.cache.lock().unwrap().insert(key.clone(), resp.clone());
        }
        self.record(&req, &resp, &key, pii_masked);
        Ok(resp)
    }
}

impl Gateway {
    fn record(&self, req: &LlmRequest, resp: &LlmResponse, key: &str, pii_masked: bool) {
        let (pi, po) = price(resp.provider, &resp.model);
        let cost = if resp.cached {
            0.0
        } else {
            (resp.input_tokens as f64 * pi + resp.output_tokens as f64 * po) / 1_000_000.0
        };
        if let Some(run) = &req.run_id {
            *self
                .run_usage
                .lock()
                .unwrap()
                .entry(run.clone())
                .or_insert(0) += resp.input_tokens + resp.output_tokens;
        }
        let entry = AuditEntry {
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
        tracing::info!(role = %entry.role, task = %entry.task, provider = %entry.provider, model = %entry.model, cached = entry.cached, cost_usd = entry.cost_usd, "llm call");
        self.audit.lock().unwrap().push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MockProvider, RouterConfig, TaskKind};
    use amap_domain::AgentRole;
    use std::sync::Arc;

    #[tokio::test]
    async fn caches_and_masks() {
        let mock = Arc::new(MockProvider::new());
        let router =
            Router::new(RouterConfig::default()).with_provider(ModelProvider::Mock, mock.clone());
        let gw = Gateway::new(router, GatewayConfig::default());
        let req = LlmRequest::new(
            TaskKind::CodeReview,
            AgentRole::Reviewer,
            "",
            "customer 900101-1234567 review this",
        )
        .with_run("RUN-1");
        let a = gw.complete(req.clone()).await.unwrap();
        let b = gw.complete(req).await.unwrap();
        assert!(!a.cached && b.cached);
        assert_eq!(mock.calls().len(), 1);
        assert!(mock.calls()[0].prompt.contains("[RRN]"));
        assert_eq!(gw.audit_log().len(), 2);
    }
}
