//! Chat Completions 클라이언트 (Azure · OpenAI 호환).
//!
//! **게이트웨이는 LLM 을 부르지 않습니다.** 이 모듈은 품질 평가의 LLM judge
//! 같은 곁가지 용도입니다. 모델 호출·대화 UI 는 게이트웨이 밖 MCP 클라이언트가
//! 담당합니다.
//!
//! 와이어 필드는 `max_completion_tokens` 입니다 — `max_tokens` 가 아닙니다.
//! gpt-5 계열이 `max_tokens` 를 400 으로 거절합니다. 의도된 선택이며 TODO 가
//! 아닙니다.

use anyhow::{Result,
             anyhow,
             bail};
use serde::{Deserialize,
            Serialize};
use serde_json::{Map,
                 Value};
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_ATTEMPTS: u32 = 3;

pub const ROLE_SYSTEM: &str = "system";
pub const ROLE_USER: &str = "user";
pub const ROLE_ASSISTANT: &str = "assistant";
pub const ROLE_TOOL: &str = "tool";

/// 대화 한 줄.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    /// `None` 과 `Some("")` 은 다릅니다 — 도구 호출만 있는 assistant 턴은
    /// `null` 이어야 합니다.
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tool_call_id: String,
}

impl Message {
    pub fn system(text: impl Into<String>) -> Self { Self::of(ROLE_SYSTEM, text) }

    pub fn user(text: impl Into<String>) -> Self { Self::of(ROLE_USER, text) }

    pub fn assistant(text: impl Into<String>) -> Self { Self::of(ROLE_ASSISTANT, text) }

    fn of(role: &str, text: impl Into<String>) -> Self {
        Self {
            role: role.to_string(),
            content: Some(text.into()),
            tool_calls: Vec::new(),
            tool_call_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    /// **JSON 문자열**입니다 (객체가 아닙니다) — OpenAI 와이어 형식 그대로.
    pub arguments: String,
}

impl FunctionCall {
    pub fn args(&self) -> Result<Map<String, Value>> {
        if self.arguments.trim().is_empty() {
            return Ok(Map::new());
        }
        Ok(serde_json::from_str::<Value>(&self.arguments)?.as_object().cloned().unwrap_or_default())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

#[derive(Debug, Clone, Default)]
pub struct Response {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    /// `stop` | `tool_calls` | `length` | `content_filter`.
    pub finish_reason: String,
    /// 모델이 거절했을 때.
    pub refusal: String,
    pub usage: Usage,
}

#[derive(Debug, Clone, Default)]
pub struct Request {
    pub messages: Vec<Message>,
    pub max_completion_tokens: i64,
    /// `low` | `medium` | `high`. 비면 보내지 않습니다.
    pub reasoning_effort: String,
}

/// 모델 제공자.
#[async_trait::async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    async fn complete(&self, req: &Request) -> Result<Response>;
    fn name(&self) -> String;
}

/// API 오류. **429·5xx 만 재시도합니다.**
#[derive(Debug, Clone, thiserror::Error)]
#[error("{provider}: HTTP {status}: {message}")]
pub struct ApiError {
    pub provider: String,
    pub status: u16,
    pub message: String,
}

impl ApiError {
    pub fn retryable(&self) -> bool { self.status == 429 || self.status >= 500 }
}

// --- 와이어 형식 ---

#[derive(Serialize)]
struct WireRequest<'a> {
    #[serde(skip_serializing_if = "str::is_empty")]
    model: &'a str,
    messages: &'a [Message],
    /// **`max_tokens` 가 아닙니다.**
    max_completion_tokens: i64,
    #[serde(skip_serializing_if = "str::is_empty")]
    reasoning_effort: &'a str,
}

#[derive(Deserialize)]
struct WireResponse {
    #[serde(default)]
    choices: Vec<WireChoice>,
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct WireChoice {
    #[serde(default)]
    finish_reason: String,
    message: WireMessage,
}

#[derive(Deserialize)]
struct WireMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    refusal: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

#[derive(Deserialize, Default)]
struct WireUsage {
    #[serde(default)]
    prompt_tokens: i64,
    #[serde(default)]
    completion_tokens: i64,
}

/// 공통 호출 경로.
async fn post(client: &reqwest::Client, url: &str, headers: Vec<(String, String)>, provider: &str, model: &str, req: &Request) -> Result<Response> {
    let max_tokens = if req.max_completion_tokens <= 0 { 1024 } else { req.max_completion_tokens };
    let body = WireRequest {
        model,
        messages: &req.messages,
        max_completion_tokens: max_tokens,
        reasoning_effort: &req.reasoning_effort,
    };

    let mut last: Option<anyhow::Error> = None;
    for attempt in 0 .. MAX_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
        }

        let mut r = client.post(url).json(&body);
        for (k, v) in &headers {
            r = r.header(k, v);
        }

        let resp = match r.send().await {
            | Ok(r) => r,
            | Err(e) => {
                last = Some(anyhow!("{provider}: {e}"));
                continue;
            },
        };

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            let message = parse_error_message(&text);
            let err = ApiError {
                provider: provider.to_string(),
                status: status.as_u16(),
                message,
            };
            if !err.retryable() {
                bail!(err);
            }
            last = Some(anyhow::Error::new(err));
            continue;
        }

        let wire: WireResponse = resp.json().await.map_err(|e| anyhow!("{provider}: decode response: {e}"))?;

        let Some(choice) = wire.choices.into_iter().next() else {
            bail!("{provider}: response had no choices");
        };
        let usage = wire.usage.unwrap_or_default();

        return Ok(Response {
            text: choice.message.content.unwrap_or_default(),
            tool_calls: choice.message.tool_calls,
            finish_reason: choice.finish_reason,
            refusal: choice.message.refusal.unwrap_or_default(),
            usage: Usage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
            },
        });
    }

    Err(last.unwrap_or_else(|| anyhow!("{provider}: exhausted retries")))
}

/// `{"error":{"message":"…"}}` 를 꺼냅니다. JSON 이 아니면 본문 그대로.
fn parse_error_message(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| body.trim().to_string())
}

// --- Azure ---

#[derive(Debug, Clone, Default)]
pub struct AzureConfig {
    pub endpoint: String,
    pub deployment: String,
    pub api_version: String,
    pub api_key: String,
}

impl AzureConfig {
    pub fn from_env() -> Result<Self> {
        let c = Self {
            endpoint: std::env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default(),
            deployment: std::env::var("AZURE_OPENAI_DEPLOYMENT").unwrap_or_default(),
            api_version: std::env::var("AZURE_OPENAI_API_VERSION").unwrap_or_default(),
            api_key: std::env::var("AZURE_OPENAI_API_KEY").unwrap_or_default(),
        };
        c.validate()?;
        Ok(c)
    }

    pub fn validate(&self) -> Result<()> {
        for (name, v) in [
            ("AZURE_OPENAI_ENDPOINT", &self.endpoint),
            ("AZURE_OPENAI_DEPLOYMENT", &self.deployment),
            ("AZURE_OPENAI_API_VERSION", &self.api_version),
            ("AZURE_OPENAI_API_KEY", &self.api_key),
        ] {
            if v.is_empty() {
                bail!("llm: {name} 이(가) 필요합니다");
            }
        }
        Ok(())
    }

    fn url(&self) -> String {
        format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            self.endpoint.trim_end_matches('/'),
            self.deployment,
            self.api_version
        )
    }
}

#[derive(Debug)]
pub struct AzureClient {
    cfg: AzureConfig,
    client: reqwest::Client,
}

impl AzureClient {
    pub fn new(cfg: AzureConfig) -> Result<Self> {
        cfg.validate()?;
        Ok(Self {
            cfg,
            client: reqwest::Client::builder().timeout(HTTP_TIMEOUT).build()?,
        })
    }
}

#[async_trait::async_trait]
impl Provider for AzureClient {
    async fn complete(&self, req: &Request) -> Result<Response> {
        post(
            &self.client,
            &self.cfg.url(),
            vec![("api-key".into(), self.cfg.api_key.clone())],
            "azure",
            // 배포 이름이 URL 에 있으므로 본문의 model 은 비웁니다.
            "",
            req,
        )
        .await
    }

    fn name(&self) -> String { self.cfg.deployment.clone() }
}

// --- OpenAI 호환 (vLLM · Ollama · OpenRouter 등) ---

#[derive(Debug, Clone, Default)]
pub struct OpenAIConfig {
    pub base_url: String,
    pub model: String,
    /// 로컬 서버는 인증을 요구하지 않을 수 있으므로 **선택**입니다.
    pub api_key: String,
}

impl OpenAIConfig {
    pub fn from_env() -> Result<Self> {
        let c = Self {
            base_url: std::env::var("OPENAI_BASE_URL").unwrap_or_default(),
            model: std::env::var("OPENAI_MODEL").unwrap_or_default(),
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        };
        c.validate()?;
        Ok(c)
    }

    pub fn validate(&self) -> Result<()> {
        if self.base_url.is_empty() {
            bail!("llm: OPENAI_BASE_URL 이 필요합니다");
        }
        if self.model.is_empty() {
            bail!("llm: OPENAI_MODEL 이 필요합니다");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct OpenAIClient {
    cfg: OpenAIConfig,
    client: reqwest::Client,
}

impl OpenAIClient {
    pub fn new(cfg: OpenAIConfig) -> Result<Self> {
        cfg.validate()?;
        Ok(Self {
            cfg,
            client: reqwest::Client::builder().timeout(HTTP_TIMEOUT).build()?,
        })
    }
}

#[async_trait::async_trait]
impl Provider for OpenAIClient {
    async fn complete(&self, req: &Request) -> Result<Response> {
        let mut headers = Vec::new();
        // **비어 있으면 헤더를 아예 보내지 않습니다** — 빈 Bearer 를 보내면 일부 로컬
        // 서버가 401 을 냅니다.
        if !self.cfg.api_key.is_empty() {
            headers.push(("Authorization".to_string(), format!("Bearer {}", self.cfg.api_key)));
        }
        post(
            &self.client,
            &format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/')),
            headers,
            "openai",
            &self.cfg.model,
            req,
        )
        .await
    }

    fn name(&self) -> String { self.cfg.model.clone() }
}

/// `LLM_PROVIDER` 환경변수로 제공자를 고릅니다 (`azure` 기본).
pub fn from_env() -> Result<Box<dyn Provider>> {
    let p = std::env::var("LLM_PROVIDER").unwrap_or_default().to_lowercase();
    match p.as_str() {
        | "" | "azure" => Ok(Box::new(AzureClient::new(AzureConfig::from_env()?)?)),
        | "openai" => Ok(Box::new(OpenAIClient::new(OpenAIConfig::from_env()?)?)),
        | other => bail!("llm: LLM_PROVIDER={other:?} 는 지원하지 않습니다"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_field_is_max_completion_tokens_not_max_tokens() {
        // gpt-5 계열이 max_tokens 를 400 으로 거절합니다.
        let msgs = vec![Message::user("안녕")];
        let body = WireRequest {
            model: "gpt-5",
            messages: &msgs,
            max_completion_tokens: 400,
            reasoning_effort: "",
        };
        let s = serde_json::to_string(&body).unwrap();
        assert!(s.contains("max_completion_tokens"));
        assert!(!s.contains("\"max_tokens\""));
        // 비어 있는 reasoning_effort 는 보내지 않습니다.
        assert!(!s.contains("reasoning_effort"));
    }

    #[test]
    fn azure_leaves_the_model_field_out_because_it_is_in_the_url() {
        let msgs = vec![Message::user("안녕")];
        let body = WireRequest {
            model: "",
            messages: &msgs,
            max_completion_tokens: 100,
            reasoning_effort: "",
        };
        let s = serde_json::to_string(&body).unwrap();
        assert!(!s.contains("\"model\""));
    }

    #[test]
    fn azure_url_shape() {
        let cfg = AzureConfig {
            endpoint: "https://x.openai.azure.com/".into(),
            deployment: "gpt-5".into(),
            api_version: "2025-01-01".into(),
            api_key: "k".into(),
        };
        assert_eq!(
            cfg.url(),
            "https://x.openai.azure.com/openai/deployments/gpt-5/chat/completions?api-version=2025-01-01"
        );
    }

    #[test]
    fn only_429_and_5xx_are_retried() {
        let e = |s| ApiError {
            provider: "azure".into(),
            status: s,
            message: String::new(),
        };
        assert!(e(429).retryable());
        assert!(e(500).retryable());
        assert!(e(503).retryable());
        assert!(!e(400).retryable());
        assert!(!e(401).retryable());
    }

    #[test]
    fn error_messages_are_extracted_from_json_or_passed_through() {
        assert_eq!(parse_error_message(r#"{"error":{"message":"bad request"}}"#), "bad request");
        assert_eq!(parse_error_message("plain text error"), "plain text error");
    }

    #[test]
    fn tool_call_arguments_are_a_json_string() {
        let f = FunctionCall {
            name: "get_invoice_status".into(),
            arguments: r#"{"invoice_id":"INV-1"}"#.into(),
        };
        assert_eq!(f.args().unwrap()["invoice_id"], serde_json::json!("INV-1"));

        let empty = FunctionCall {
            name: "x".into(),
            arguments: String::new(),
        };
        assert!(empty.args().unwrap().is_empty());
    }

    #[test]
    fn a_tool_only_assistant_turn_has_null_content() {
        // None 과 Some("") 은 다릅니다.
        let m = Message {
            role: ROLE_ASSISTANT.into(),
            content: None,
            tool_calls: vec![],
            tool_call_id: String::new(),
        };
        let s = serde_json::to_string(&m).unwrap();
        assert!(s.contains("\"content\":null"));
    }

    #[test]
    fn configs_reject_missing_required_fields() {
        assert!(AzureConfig::default().validate().is_err());
        assert!(OpenAIConfig::default().validate().is_err());
        // OpenAI 는 api_key 없이도 유효합니다 (로컬 서버).
        let c = OpenAIConfig {
            base_url: "http://localhost:11434/v1".into(),
            model: "llama3".into(),
            api_key: String::new(),
        };
        assert!(c.validate().is_ok());
    }
}
