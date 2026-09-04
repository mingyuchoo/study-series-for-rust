//! Anthropic Messages API over raw HTTP (no official Rust SDK).
use crate::{Effort, LlmClient, LlmError, LlmRequest, LlmResponse};
use amap_domain::ModelProvider;
use async_trait::async_trait;
use serde_json::{json, Value};

pub const DEFAULT_MODEL: &str = "claude-opus-5";

pub struct AnthropicProvider {
    http: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .expect("http client"),
            api_key: api_key.into(),
            model: DEFAULT_MODEL.into(),
            base_url: "https://api.anthropic.com".into(),
        }
    }
    pub fn with_model(mut self, m: impl Into<String>) -> Self {
        self.model = m.into();
        self
    }
    pub fn with_base_url(mut self, u: impl Into<String>) -> Self {
        self.base_url = u.into();
        self
    }
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("ANTHROPIC_API_KEY").ok()?;
        let mut p = Self::new(key);
        if let Ok(m) = std::env::var("AMAP_ANTHROPIC_MODEL") {
            p = p.with_model(m);
        }
        if let Ok(u) = std::env::var("ANTHROPIC_BASE_URL") {
            p = p.with_base_url(u);
        }
        Some(p)
    }
}

fn effort_str(e: Effort) -> &'static str {
    match e {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High => "high",
        Effort::Xhigh => "xhigh",
        Effort::Max => "max",
    }
}

#[async_trait]
impl LlmClient for AnthropicProvider {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let mut output_config = json!({ "effort": effort_str(req.effort) });
        if let Some(schema) = &req.schema {
            output_config["format"] = json!({ "type": "json_schema", "schema": schema });
        }
        let body = json!({
            "model": self.model,
            "max_tokens": req.max_tokens,
            "system": req.system,
            "messages": [{ "role": "user", "content": req.prompt }],
            "thinking": { "type": "adaptive" },
            "output_config": output_config,
            // Server-side refusal fallback: route by refusal category to a suitable model.
            "fallbacks": "default",
        });
        let resp = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("content-type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "server-side-fallback-2026-07-01")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if status.as_u16() == 429 || status.as_u16() >= 500 {
            return Err(LlmError::Transient(format!("{status}: {text}")));
        }
        if !status.is_success() {
            return Err(LlmError::Provider {
                status: status.as_u16(),
                body: text,
            });
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| LlmError::Invalid(e.to_string()))?;
        let stop_reason = v["stop_reason"].as_str().unwrap_or("").to_string();
        if stop_reason == "refusal" {
            return Err(LlmError::Refused(v["stop_details"].to_string()));
        }
        let out: String = v["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();
        let json = if req.schema.is_some() {
            crate::extract_json(&out)
        } else {
            None
        };
        Ok(LlmResponse {
            text: out,
            json,
            provider: ModelProvider::Anthropic,
            model: v["model"].as_str().unwrap_or(&self.model).to_string(),
            input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0),
            output_tokens: v["usage"]["output_tokens"].as_u64().unwrap_or(0),
            stop_reason,
            cached: false,
        })
    }
}
