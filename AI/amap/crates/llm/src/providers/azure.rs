//! Azure OpenAI chat completions. Unlike the OpenAI-compatible provider, Azure addresses a
//! *deployment* (`/openai/deployments/{name}/...`), requires an `api-version` query parameter
//! and authenticates with an `api-key` header instead of a bearer token.
use crate::{Effort, LlmClient, LlmError, LlmRequest, LlmResponse};
use amap_domain::ModelProvider;
use async_trait::async_trait;
use serde_json::{json, Value};

pub const DEFAULT_API_VERSION: &str = "2024-12-01-preview";

pub struct AzureOpenAiProvider {
    http: reqwest::Client,
    api_key: String,
    /// Resource endpoint, e.g. `https://my-resource.cognitiveservices.azure.com` (no trailing slash).
    endpoint: String,
    /// Deployment used for `Effort::High` and above (and for every effort when no tier is set).
    deployment: String,
    /// Optional cheaper deployments for lower reasoning efforts (e.g. `gpt-5.4-nano`, `gpt-5.4-mini`).
    deployment_low: Option<String>,
    deployment_medium: Option<String>,
    api_version: String,
}

impl AzureOpenAiProvider {
    /// `deployment` is the Azure deployment name (e.g. `gpt-5.4`), not the underlying model id.
    pub fn new(
        api_key: impl Into<String>,
        endpoint: impl Into<String>,
        deployment: impl Into<String>,
    ) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .expect("http client"),
            api_key: api_key.into(),
            endpoint: endpoint.into().trim_end_matches('/').to_string(),
            deployment: deployment.into(),
            deployment_low: None,
            deployment_medium: None,
            api_version: DEFAULT_API_VERSION.into(),
        }
    }
    pub fn with_api_version(mut self, v: impl Into<String>) -> Self {
        self.api_version = v.into();
        self
    }
    /// Deployment used when the request effort is `Low`.
    pub fn with_low_deployment(mut self, d: impl Into<String>) -> Self {
        self.deployment_low = Some(d.into());
        self
    }
    /// Deployment used when the request effort is `Medium`.
    pub fn with_medium_deployment(mut self, d: impl Into<String>) -> Self {
        self.deployment_medium = Some(d.into());
        self
    }

    /// Reads `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_ENDPOINT` and `AZURE_OPENAI_DEPLOYMENT`
    /// (all required), plus optional `AZURE_OPENAI_API_VERSION`,
    /// `AZURE_OPENAI_DEPLOYMENT_LOW` and `AZURE_OPENAI_DEPLOYMENT_MEDIUM`.
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("AZURE_OPENAI_API_KEY").ok()?;
        let endpoint = std::env::var("AZURE_OPENAI_ENDPOINT").ok()?;
        let deployment = std::env::var("AZURE_OPENAI_DEPLOYMENT").ok()?;
        let mut p = Self::new(key, endpoint, deployment);
        if let Ok(v) = std::env::var("AZURE_OPENAI_API_VERSION") {
            p = p.with_api_version(v);
        }
        if let Ok(d) = std::env::var("AZURE_OPENAI_DEPLOYMENT_LOW") {
            p = p.with_low_deployment(d);
        }
        if let Ok(d) = std::env::var("AZURE_OPENAI_DEPLOYMENT_MEDIUM") {
            p = p.with_medium_deployment(d);
        }
        Some(p)
    }

    fn deployment_for(&self, effort: Effort) -> &str {
        match effort {
            Effort::Low => self
                .deployment_low
                .as_deref()
                .or(self.deployment_medium.as_deref())
                .unwrap_or(&self.deployment),
            Effort::Medium => self
                .deployment_medium
                .as_deref()
                .unwrap_or(&self.deployment),
            Effort::High | Effort::Xhigh | Effort::Max => &self.deployment,
        }
    }

    fn chat_url(&self, deployment: &str) -> String {
        format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            self.endpoint, deployment, self.api_version
        )
    }
}

#[async_trait]
impl LlmClient for AzureOpenAiProvider {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let deployment = self.deployment_for(req.effort).to_string();
        let mut body = json!({
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
            .post(self.chat_url(&deployment))
            .header("api-key", &self.api_key)
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
            provider: ModelProvider::Azure,
            // Azure reports the underlying model id; fall back to the deployment name.
            model: v["model"].as_str().unwrap_or(&deployment).to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> AzureOpenAiProvider {
        AzureOpenAiProvider::new(
            "k",
            "https://example.cognitiveservices.azure.com/",
            "gpt-5.4",
        )
    }

    #[test]
    fn builds_deployment_scoped_url_without_double_slash() {
        assert_eq!(
            provider().chat_url("gpt-5.4"),
            "https://example.cognitiveservices.azure.com/openai/deployments/gpt-5.4/chat/completions?api-version=2024-12-01-preview"
        );
    }

    #[test]
    fn single_deployment_serves_every_effort() {
        let p = provider();
        for e in [
            Effort::Low,
            Effort::Medium,
            Effort::High,
            Effort::Xhigh,
            Effort::Max,
        ] {
            assert_eq!(p.deployment_for(e), "gpt-5.4");
        }
    }

    #[test]
    fn effort_tiers_select_cheaper_deployments() {
        let p = provider()
            .with_low_deployment("gpt-5.4-nano")
            .with_medium_deployment("gpt-5.4-mini");
        assert_eq!(p.deployment_for(Effort::Low), "gpt-5.4-nano");
        assert_eq!(p.deployment_for(Effort::Medium), "gpt-5.4-mini");
        assert_eq!(p.deployment_for(Effort::High), "gpt-5.4");
        assert_eq!(p.deployment_for(Effort::Max), "gpt-5.4");
        // Low falls back to medium when only medium is configured.
        let p = provider().with_medium_deployment("gpt-5.4-mini");
        assert_eq!(p.deployment_for(Effort::Low), "gpt-5.4-mini");
    }

    #[test]
    fn default_api_version_matches_azure_preview() {
        assert_eq!(provider().api_version, DEFAULT_API_VERSION);
        assert_eq!(DEFAULT_API_VERSION, "2024-12-01-preview");
    }
}
