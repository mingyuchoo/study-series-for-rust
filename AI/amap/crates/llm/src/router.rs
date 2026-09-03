//! Role → provider routing (design §26): builder and verifier use different models when possible.
use crate::{LlmClient, LlmError, LlmRequest, LlmResponse};
use amap_domain::{AgentRole, ModelProvider};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Ordered preference per role; the first configured provider wins.
    pub routes: HashMap<AgentRole, Vec<ModelProvider>>,
    /// Used when no route matches or no preferred provider is configured.
    pub fallback: Vec<ModelProvider>,
}

impl Default for RouterConfig {
    fn default() -> Self {
        use AgentRole::*;
        use ModelProvider::*;
        let mut routes = HashMap::new();
        routes.insert(Orchestrator, vec![OpenAi, Anthropic]);
        routes.insert(Builder, vec![OpenAi, Anthropic]);
        routes.insert(Fix, vec![OpenAi, Anthropic]);
        routes.insert(Discovery, vec![Anthropic, OpenAi]);
        routes.insert(RuleMiner, vec![Anthropic, OpenAi]);
        routes.insert(BehaviorMiner, vec![Anthropic, OpenAi]);
        routes.insert(Architecture, vec![Anthropic, OpenAi]);
        routes.insert(TestGenerator, vec![Anthropic, OpenAi]);
        routes.insert(Adversarial, vec![Anthropic, OpenAi]); // different from Builder
        routes.insert(Reviewer, vec![Anthropic, OpenAi]); // different from Builder
        routes.insert(BusinessReviewer, vec![Anthropic, OpenAi]);
        routes.insert(Rca, vec![OpenAi, Anthropic]);
        Self { routes, fallback: vec![Anthropic, OpenAi, Local, Mock] }
    }
}

pub struct Router {
    config: RouterConfig,
    providers: HashMap<ModelProvider, Arc<dyn LlmClient>>,
}

impl Router {
    pub fn new(config: RouterConfig) -> Self {
        Self { config, providers: HashMap::new() }
    }
    pub fn with_provider(mut self, p: ModelProvider, client: Arc<dyn LlmClient>) -> Self {
        self.providers.insert(p, client);
        self
    }
    pub fn has(&self, p: ModelProvider) -> bool {
        self.providers.contains_key(&p)
    }
    pub fn configured(&self) -> Vec<ModelProvider> {
        self.providers.keys().copied().collect()
    }

    /// Resolve the provider for a request.
    pub fn resolve(&self, req: &LlmRequest) -> Result<ModelProvider, LlmError> {
        if let Some(p) = req.provider {
            return if self.has(p) { Ok(p) } else { Err(LlmError::NotConfigured(p)) };
        }
        let prefs = self.config.routes.get(&req.role).cloned().unwrap_or_default();
        prefs
            .iter()
            .chain(self.config.fallback.iter())
            .find(|p| self.has(**p))
            .copied()
            .ok_or(LlmError::NotConfigured(prefs.first().copied().unwrap_or(ModelProvider::Anthropic)))
    }

    pub fn provider(&self, p: ModelProvider) -> Option<Arc<dyn LlmClient>> {
        self.providers.get(&p).cloned()
    }
}

#[async_trait]
impl LlmClient for Router {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let p = self.resolve(&req)?;
        self.providers[&p].complete(req).await
    }
}
