//! LLM access layer (stack §13): agents call `llm.complete(task)`; the gateway handles
//! routing, prompt versions, PII filtering, caching, retries, budgets, audit and cost.
//! Vendors are swappable execution engines behind [`LlmClient`].

pub mod client;
pub mod gateway;
pub mod pii;
pub mod prompts;
pub mod providers;
pub mod router;

pub use client::GatewayClient;
pub use gateway::{AuditEntry, Gateway, GatewayConfig};
pub use providers::{anthropic::AnthropicProvider, mock::MockProvider, openai::OpenAiProvider};
pub use router::{Router, RouterConfig};

/// Provider-neutral LLM request, response, and policy contracts.
pub mod core {
    pub use crate::{Effort, LlmClient, LlmError, LlmRequest, LlmResponse, TaskKind};
}

/// Network and provider adapters plus the stateful gateway shell.
pub mod adapters {
    pub use crate::{
        AnthropicProvider, Gateway, GatewayClient, GatewayConfig, MockProvider, OpenAiProvider,
        Router, RouterConfig,
    };
}

use amap_domain::{AgentRole, ModelProvider};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    DomainClassification,
    BusinessRuleExtraction,
    BehaviorLinking,
    ArchitectureMapping,
    CodeGeneration,
    TestGeneration,
    AdversarialProbe,
    RootCauseAnalysis,
    PatchGeneration,
    CodeReview,
    BusinessReview,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    #[default]
    High,
    Xhigh,
    Max,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmRequest {
    pub task: TaskKind,
    pub role: AgentRole,
    pub system: String,
    pub prompt: String,
    /// JSON schema for structured output (when set, `LlmResponse::json` is populated).
    #[serde(default)]
    pub schema: Option<Value>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub effort: Effort,
    /// Force a provider (used for N-version reasoning and builder/verifier diversification).
    #[serde(default)]
    pub provider: Option<ModelProvider>,
    #[serde(default)]
    pub run_id: Option<String>,
}

fn default_max_tokens() -> u32 {
    16_000
}

impl LlmRequest {
    pub fn new(
        task: TaskKind,
        role: AgentRole,
        system: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            task,
            role,
            system: system.into(),
            prompt: prompt.into(),
            schema: None,
            max_tokens: 16_000,
            effort: Effort::High,
            provider: None,
            run_id: None,
        }
    }
    pub fn with_schema(mut self, schema: Value) -> Self {
        self.schema = Some(schema);
        self
    }
    pub fn with_provider(mut self, p: ModelProvider) -> Self {
        self.provider = Some(p);
        self
    }
    pub fn with_run(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }
    pub fn with_effort(mut self, e: Effort) -> Self {
        self.effort = e;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmResponse {
    pub text: String,
    #[serde(default)]
    pub json: Option<Value>,
    pub provider: ModelProvider,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub stop_reason: String,
    #[serde(default)]
    pub cached: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("provider {0:?} is not configured")]
    NotConfigured(ModelProvider),
    #[error("request refused by safety policy: {0}")]
    Refused(String),
    #[error("token budget exceeded for run {0}")]
    BudgetExceeded(String),
    #[error("transient error: {0}")]
    Transient(String),
    #[error("provider error {status}: {body}")]
    Provider { status: u16, body: String },
    #[error("invalid response: {0}")]
    Invalid(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError>;
}

/// Extract the first JSON object/array from free text (for providers without structured output).
pub fn extract_json(text: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
        return Some(v);
    }
    let start = text.find(['{', '['])?;
    let bytes = text.as_bytes();
    let open = bytes[start];
    let close = if open == b'{' { b'}' } else { b']' };
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            _ if b == open => depth += 1,
            _ if b == close => {
                depth -= 1;
                if depth == 0 {
                    return serde_json::from_str(&text[start..=i]).ok();
                }
            }
            _ => {}
        }
    }
    None
}
