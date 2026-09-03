//! HTTP client for the standalone LLM Gateway service (`services/llm-gateway`).
use crate::{LlmClient, LlmError, LlmRequest, LlmResponse};
use async_trait::async_trait;

pub struct GatewayClient {
    http: reqwest::Client,
    base_url: String,
}

impl GatewayClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { http: reqwest::Client::builder().timeout(std::time::Duration::from_secs(900)).build().expect("http client"), base_url: base_url.into() }
    }
}

#[async_trait]
impl LlmClient for GatewayClient {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let resp = self.http.post(format!("{}/v1/complete", self.base_url.trim_end_matches('/'))).json(&req).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if status.as_u16() >= 500 {
            return Err(LlmError::Transient(text));
        }
        if !status.is_success() {
            return Err(LlmError::Provider { status: status.as_u16(), body: text });
        }
        serde_json::from_str(&text).map_err(|e| LlmError::Invalid(e.to_string()))
    }
}
