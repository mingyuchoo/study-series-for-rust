//! 레거시 시스템 어댑터 **계약**과 스키마 헬퍼.
//!
//! 구현체(ERP·CRM·티켓 등)와 Transport 는 `legacies` 크레이트에 있습니다. 이
//! 모듈은 게이트웨이가 어댑터를 레지스트리·MCP·운영 콘솔에 붙일 때 쓰는 경계만
//! 담당합니다.
//!
//! 도구의 입력과 출력은 **모두** JSON Schema로 선언해야 합니다. 입력 스키마는
//! LLM이 만든 인자를 통제하고, 출력 스키마는 레거시 응답이 계약을 지키는지
//! 검증합니다.

use crate::{auth::Identity,
            registry::{self,
                       Handler,
                       Tool}};
use anyhow::{Result,
             anyhow,
             bail};
use serde_json::{Map,
                 Value,
                 json};
use std::time::Duration;

/// 하나의 레거시 시스템을 표준 도구 집합으로 감싸는 어댑터.
#[async_trait::async_trait]
pub trait Adapter: Send + Sync {
    /// 레거시 시스템 이름. 예: `erp`, `crm`.
    fn name(&self) -> String;

    /// 이 어댑터가 제공하는 도구 목록.
    fn tools(&self) -> Vec<Tool>;

    /// 레거시 시스템에 도달 가능한지 확인합니다.
    async fn health_check(&self) -> Result<()>;

    /// MCP Resource 로 노출할 읽기 전용 자원 (선택).
    fn resources(&self) -> Vec<Resource> { Vec::new() }
}

/// MCP Resource — 도구가 "행동"이라면 자원은 "참조 대상"입니다.
///
/// 사내 규정 원문처럼 LLM이 도구 호출 없이 직접 읽어도 되는 것들입니다.
/// 안정적인 URI를 부여하면 검색 결과의 근거 링크가 그 URI를 가리킬 수 있습니다.
#[derive(Clone)]
pub struct Resource {
    /// 안정적인 식별자. 예: `docs://DOC-001`.
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
    /// 본문을 읽습니다. **호출 주체를 받으므로 도구와 동일한 행 수준 접근
    /// 제어를 적용합니다.**
    pub read: std::sync::Arc<dyn ResourceReader>,
}

impl std::fmt::Debug for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("Resource").field("uri", &self.uri).field("name", &self.name).finish() }
}

#[async_trait::async_trait]
pub trait ResourceReader: Send + Sync {
    async fn read(&self, id: &Identity) -> Result<String>;
}

// ---------------------------------------------------------------------------
// RAG 검색기
// ---------------------------------------------------------------------------

/// 문서 조각.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chunk {
    pub doc_id: String,
    pub chunk_id: String,
    pub title: String,
    pub text: String,
    /// 이 조각을 볼 수 있는 역할. 비면 제한 없음.
    pub roles: Vec<String>,
    /// 이 조각이 속한 고객 (담당자만 볼 수 있음). 비면 제한 없음.
    pub customer_id: String,
    pub uri: String,
}

/// 검색 결과 한 건.
#[derive(Debug, Clone)]
pub struct Hit {
    pub chunk: Chunk,
    pub score: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Query {
    pub text: String,
    pub top_k: usize,
}

/// 교체 가능한 검색기.
///
/// **권한 필터(`allow`)는 검색기 밖에 있고, 점수를 매기기 전에 적용됩니다.**
/// 필터를 검색 뒤에 걸면 상위 K건이 전부 걸러져 빈 결과가 나옵니다. 벡터 DB에
/// 접근 제어를 맡기면 엔진을 바꿀 때마다 그것을 다시 구현해야 합니다.
///
/// 검색기가 죽으면 빈 결과가 아니라 **오류**를 올려야 합니다. 빈 결과를
/// 돌려주면 LLM은 "그런 규정이 없다"고 답합니다.
/// 권한 술어. `Sync` 가 필요합니다 — 그러지 않으면 검색 future 를 스레드 간에
/// 보낼 수 없어 async 런타임에 올릴 수 없습니다.
pub type AllowFn<'a> = &'a (dyn Fn(&Chunk) -> bool + Sync);

#[async_trait::async_trait]
pub trait Retriever: Send + Sync + std::fmt::Debug {
    async fn index(&self, chunks: Vec<Chunk>) -> Result<()>;
    async fn search(&self, q: &Query, allow: AllowFn<'_>) -> Result<Vec<Hit>>;
}

// ---------------------------------------------------------------------------
// 스키마 헬퍼
// ---------------------------------------------------------------------------

/// 닫힌 object 스키마. **선언하지 않은 필드는 거부합니다** — LLM이 지어낸
/// 인자를 조용히 통과시키지 않기 위함입니다.
pub fn object(props: Vec<(&str, Value)>, required: &[&str]) -> Value {
    let mut properties = Map::new();
    for (k, v) in props {
        properties.insert(k.to_string(), v);
    }
    let mut s = json!({
        "type": "object",
        "properties": Value::Object(properties),
        "additionalProperties": false,
    });
    if !required.is_empty() {
        s["required"] = json!(required);
    }
    s
}

pub fn str_prop(desc: &str) -> Value { json!({"type": "string", "description": desc}) }

pub fn str_enum(desc: &str, values: &[&str]) -> Value { json!({"type": "string", "description": desc, "enum": values}) }

pub fn int_prop(desc: &str) -> Value { json!({"type": "integer", "description": desc}) }

pub fn bool_prop(desc: &str) -> Value { json!({"type": "boolean", "description": desc}) }

pub fn num_prop(desc: &str) -> Value { json!({"type": "number", "description": desc}) }

pub fn array_prop(desc: &str, items: Value) -> Value { json!({"type": "array", "description": desc, "items": items}) }

// ---------------------------------------------------------------------------
// remote 브리지 (동적 도구 · 원격 어댑터)
// ---------------------------------------------------------------------------

const REMOTE_TIMEOUT: Duration = Duration::from_secs(10);

/// 원격 REST 브리지 핸들러.
///
/// 와이어 계약: `POST {base_url}/tools/{tool}` — 본문은 인자 JSON, 헤더는
/// `X-User-Id`. **호출 주체 중 user_id 만 넘어갑니다** — 역할·권한·속성은
/// 게이트웨이 안에 머뭅니다.
///
/// 재시도·백오프는 여기 없습니다. 그것은 전적으로 게이트웨이 파이프라인의
/// 몫입니다.
pub fn remote_handler(base_url: &str, tool_name: &str) -> Result<Handler> {
    if tool_name.is_empty() {
        bail!("tool name is required");
    }
    let base = validate_base_url(base_url)?;
    let tool = tool_name.to_string();
    let client = reqwest::Client::builder().timeout(REMOTE_TIMEOUT).build()?;

    Ok(registry::handler(move |id: Identity, args: Map<String, Value>| {
        let base = base.clone();
        let tool = tool.clone();
        let client = client.clone();
        async move { invoke(&client, &base, &tool, &id, &args).await }
    }))
}

fn validate_base_url(base_url: &str) -> Result<String> {
    let u = reqwest::Url::parse(base_url).map_err(|_| anyhow!("invalid REST base_url {base_url:?}"))?;
    if u.scheme().is_empty() || u.host_str().is_none() {
        bail!("invalid REST base_url {base_url:?}");
    }
    Ok(base_url.trim_end_matches('/').to_string())
}

async fn invoke(client: &reqwest::Client, base: &str, tool: &str, id: &Identity, args: &Map<String, Value>) -> Result<Value> {
    let url = format!("{base}/tools/{}", urlencode(tool));
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header(crate::auth::USER_ID_HEADER, &id.user_id)
        .json(args)
        .send()
        .await
        // 네트워크 실패는 일시적 장애입니다 — 게이트웨이가 재시도·브레이커에 반영합니다.
        .map_err(crate::transient::temporary)?;

    let status = resp.status();
    if !status.is_success() {
        let err = anyhow!("legacy REST {tool} returned {status}");
        // 5xx·429 는 재시도할 가치가 있고, 4xx 는 업무 오류입니다.
        if status.is_server_error() || status.as_u16() == 429 {
            return Err(crate::transient::temporary(err));
        }
        return Err(err);
    }
    resp.json::<Value>().await.map_err(|e| anyhow!("decode legacy REST response: {e}"))
}

fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.~".contains(c) {
                c.to_string()
            } else {
                c.to_string().as_bytes().iter().map(|b| format!("%{b:02X}")).collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_is_closed_by_default() {
        let s = object(vec![("invoice_id", str_prop("송장 번호"))], &["invoice_id"]);
        assert_eq!(s["additionalProperties"], json!(false));
        assert_eq!(s["required"], json!(["invoice_id"]));
    }

    #[test]
    fn object_omits_required_when_empty() {
        let s = object(vec![("q", str_prop("검색어"))], &[]);
        assert!(s.get("required").is_none());
    }

    #[test]
    fn remote_handler_rejects_bad_base_url() {
        assert!(remote_handler("not-a-url", "t").is_err());
        assert!(remote_handler("https://x.example", "").is_err());
        assert!(remote_handler("https://x.example", "t").is_ok());
    }

    #[test]
    fn urlencode_escapes_path_segments() {
        assert_eq!(urlencode("get_invoice"), "get_invoice");
        assert_eq!(urlencode("a/b"), "a%2Fb");
    }
}
