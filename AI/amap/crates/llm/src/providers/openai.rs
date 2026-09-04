//! OpenAI-compatible chat completions (also used for local OpenAI-compatible servers).
use crate::{LlmClient, LlmError, LlmRequest, LlmResponse};
use amap_domain::ModelProvider;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct OpenAiProvider {
    http: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
    provider: ModelProvider,
}

impl OpenAiProvider {
    /// `model` must be the deployed model id (e.g. the GPT-5.6 Sol id used by the engagement).
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .expect("http client"),
            api_key: api_key.into(),
            model: model.into(),
            base_url: "https://api.openai.com".into(),
            provider: ModelProvider::OpenAi,
        }
    }
    pub fn with_base_url(mut self, u: impl Into<String>) -> Self {
        self.base_url = u.into();
        self
    }
    /// Mark as a local model server (vLLM / Ollama OpenAI-compatible endpoint).
    pub fn local(mut self) -> Self {
        self.provider = ModelProvider::Local;
        self
    }
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("OPENAI_API_KEY").ok()?;
        let model = std::env::var("AMAP_OPENAI_MODEL").ok()?;
        let mut p = Self::new(key, model);
        if let Ok(u) = std::env::var("OPENAI_BASE_URL") {
            p = p.with_base_url(u);
        }
        Some(p)
    }
}

#[async_trait]
impl LlmClient for OpenAiProvider {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let mut body = json!({
            "model": self.model,
            "max_completion_tokens": req.max_tokens,
            "messages": [
                { "role": "system", "content": req.system },
                { "role": "user", "content": req.prompt }
            ]
        });
        if let Some(schema) = &req.schema {
            body["response_format"] = json!({ "type": "json_schema", "json_schema": { "name": "amap_output", "schema": schema } });
        }
        let resp = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
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
        let out = v["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let json = if req.schema.is_some() {
            crate::extract_json(&out)
        } else {
            None
        };
        Ok(LlmResponse {
            text: out,
            json,
            provider: self.provider,
            model: v["model"].as_str().unwrap_or(&self.model).to_string(),
            input_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
            output_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
            stop_reason: v["choices"][0]["finish_reason"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            cached: false,
        })
    }
}
