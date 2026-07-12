//! MCP 서버 — 레지스트리를 MCP Tools/Resources 로 노출합니다.
//!
//! **동적 도구 목록.** 레지스트리는 핫 리로드되므로 도구 목록을 컴파일 시점에
//! 고정할 수 없습니다. 그래서 `rmcp` 의 매크로 대신 [`ServerHandler`] 를 직접
//! 구현합니다.
//!
//! # 위험 등급은 설명 문자열이 아니라 표준 annotation 으로 전달됩니다
//!
//! | 등급 | `readOnlyHint` | `destructiveHint` |
//! |---|---|---|
//! | L1 조회 | true | false |
//! | L2 초안 | false | false |
//! | L3 실행 | false | **true** |
//! | L4 자동 | false | **true** |
//!
//! 물론 **실제 통제는 게이트웨이가 합니다.** 클라이언트가 힌트를 무시해도 승인
//! 관문은 그대로 동작합니다.
//!
//! # 관측은 통제가 아닙니다
//!
//! 게이트웨이는 LLM 을 부르지 않으므로 "무엇을 물어봐서 이 도구가 불렸는지"를
//! 스스로 알 수 없습니다. 오케스트레이터가 `_meta` 에 실어 보내면 감사 로그와
//! 비용 상한이 그 값을 씁니다. **보내지 않아도 도구는 정상 동작합니다** — 관측
//! 정보가 빠졌다고 호출을 거부하지 않습니다.

use crate::{adapter::Resource,
            auth::{self,
                   Identity,
                   RequestContext as AuthContext,
                   SharedResolver},
            gateway::{Call,
                      CallResult,
                      Gateway,
                      Usage},
            registry::{Access,
                       RiskLevel,
                       Spec}};
use rmcp::{ErrorData as McpError,
           RoleServer,
           ServerHandler,
           model::{CallToolRequestParams,
                   CallToolResult,
                   ContentBlock,
                   Implementation,
                   ListResourcesResult,
                   ListToolsResult,
                   PaginatedRequestParams,
                   ProtocolVersion,
                   ReadResourceRequestParams,
                   ReadResourceResult,
                   ResourceContents,
                   ServerCapabilities,
                   ServerInfo,
                   Tool,
                   ToolAnnotations},
           service::RequestContext};
use serde_json::{Map,
                 Value,
                 json};
use std::sync::Arc;

/// 오케스트레이터가 실어 보내는 관측 정보.
pub const META_PROMPT: &str = "ai.bridge/prompt";
pub const META_INPUT_TOKENS: &str = "ai.bridge/input_tokens";
pub const META_OUTPUT_TOKENS: &str = "ai.bridge/output_tokens";
pub const META_COST_MICROS: &str = "ai.bridge/cost_micros";

/// 결과에 붙는 판단 필드. 도구 업무 데이터와 충돌하지 않도록 `_` 로 시작합니다.
pub const FIELD_MASKED: &str = "_masked";
pub const FIELD_NARROWED: &str = "_narrowed";
pub const FIELD_REQUEST_ID: &str = "_request_id";

/// MCP 서버.
#[derive(Clone)]
pub struct Server {
    gw: Arc<Gateway>,
    resolve: SharedResolver,
    resources: Arc<Vec<Resource>>,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("mcpserver::Server").finish() }
}

impl Server {
    pub fn new(gw: Arc<Gateway>, resolve: SharedResolver, resources: Vec<Resource>) -> Self {
        Self {
            gw,
            resolve,
            resources: Arc::new(resources),
        }
    }

    /// 요청에서 호출 주체를 해석합니다.
    ///
    /// **stdio 는 헤더가 없으므로** `StaticResolver` 가 고정 주체를 돌려줍니다.
    /// **HTTP 는** 전송이 넣어준 `http::request::Parts` 에서 헤더를 꺼냅니다.
    fn identity(&self, ctx: &RequestContext<RoleServer>) -> Result<Identity, String> {
        let mut rc = AuthContext {
            now: Some(chrono::Utc::now()),
            ..Default::default()
        };

        if let Some(parts) = ctx.extensions.get::<http::request::Parts>() {
            for (k, v) in parts.headers.iter() {
                if let Ok(s) = v.to_str() {
                    rc.set(k.as_str(), s);
                }
            }
            // 세션 ID 는 헤더가 우선입니다.
            let sid = rc.get(auth::SESSION_ID_HEADER).to_string();
            if !sid.is_empty() {
                rc.session_id = sid;
            }
        }

        self.resolve.resolve(&rc).map_err(|e| e.to_string())
    }
}

/// `_meta` 에서 관측 정보를 꺼냅니다. **없거나 형식이 틀려도 거부하지
/// 않습니다.**
///
/// rmcp 는 `_meta` 를 params 에서 꺼내 **`RequestContext.meta` 로 옮깁니다.**
/// 그래서 `CallToolRequestParams.meta` 는 언제나 `None` 입니다 — 거기서 읽으면
/// 프롬프트·토큰·비용이 조용히 사라집니다(감사 로그와 비용 상한이
/// 무력화됩니다).
fn observability(meta: Option<&rmcp::model::Meta>) -> (String, Usage) {
    let Some(m) = meta else {
        return (String::new(), Usage::default());
    };
    let get_i64 = |k: &str| -> i64 { m.get(k).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))).unwrap_or(0) };
    let prompt = m.get(META_PROMPT).and_then(|v| v.as_str()).unwrap_or("").to_string();

    (
        prompt,
        Usage {
            input_tokens: get_i64(META_INPUT_TOKENS),
            output_tokens: get_i64(META_OUTPUT_TOKENS),
            cost_micros: get_i64(META_COST_MICROS),
        },
    )
}

/// 위험 등급 → MCP 표준 annotation.
fn annotations(spec: &Spec) -> ToolAnnotations {
    let read_only = spec.access == Access::Read;
    ToolAnnotations::with_title(spec.name.clone())
        .read_only(read_only)
        // **L2(초안)는 파괴적이지 않습니다.** 되돌리기 어려운 것은 L3 부터입니다.
        .destructive(spec.risk_level >= RiskLevel::L3)
        .idempotent(read_only)
        // 레거시는 닫힌 세계입니다.
        .open_world(false)
}

/// 설명에 메타데이터를 덧붙여 사람이 읽을 수 있게 합니다.
fn annotated_description(spec: &Spec) -> String {
    let mut tags = vec![
        format!("시스템={}", spec.system),
        format!("접근={}", spec.access),
        format!("위험={}", spec.risk_level),
        format!("민감도={}", spec.sensitivity),
    ];
    if spec.risk_level >= RiskLevel::L4 {
        tags.push("기본차단".into());
    }
    if spec.risk_level >= RiskLevel::L3 {
        tags.push("승인필요".into());
    }
    format!("{} [{}]", spec.description, tags.join(", "))
}

fn to_json_object(v: &Value) -> Map<String, Value> { v.as_object().cloned().unwrap_or_default() }

/// 결과에 판단 필드를 붙입니다.
fn annotate(res: &CallResult) -> Map<String, Value> {
    let mut payload = res.data.clone();
    payload.insert(FIELD_MASKED.into(), json!(res.masked));
    payload.insert(FIELD_NARROWED.into(), json!(res.narrowed));
    payload.insert(FIELD_REQUEST_ID.into(), json!(res.request_id));
    payload
}

/// 게이트웨이 오류를 **툴 결과**로 돌려줍니다.
///
/// MCP 프로토콜 오류가 아니라 `isError: true` 인 툴 결과입니다 — 그래야 LLM 이
/// 오류를 읽고 스스로 교정할 수 있습니다. 프로토콜 오류로 돌려주면 클라이언트가
/// "내부 오류"라고만 표시하고 우리 메시지는 사용자에게 닿지 않습니다.
fn error_result(code: &str, message: &str, fallback: &str) -> CallToolResult {
    let mut structured = Map::new();
    structured.insert("error_code".into(), json!(code));
    structured.insert("error_message".into(), json!(message));
    if !fallback.is_empty() {
        structured.insert("fallback".into(), json!(fallback));
    }

    let text = if fallback.is_empty() {
        format!("[{code}] {message}")
    } else {
        format!("[{code}] {message}\n대안: {fallback}")
    };

    let mut r = CallToolResult::error(vec![ContentBlock::text(text)]);
    r.structured_content = Some(Value::Object(structured));
    r
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        let mut info = Implementation::new("ai-bridge", env!("CARGO_PKG_VERSION"));
        info.title = Some("AI Integration Gateway".into());

        // `ServerInfo` 는 non-exhaustive 라 구조체 리터럴로 만들 수 없습니다.
        let mut out = ServerInfo::default();
        out.protocol_version = ProtocolVersion::LATEST;
        out.capabilities = ServerCapabilities::builder().enable_tools().enable_resources().build();
        out.server_info = info;
        out.instructions = Some(
            "레거시 시스템을 안전하게 호출하는 게이트웨이입니다. 모든 호출은 정책·승인·\
             감사를 통과합니다. 프롬프트·토큰·비용은 _meta(ai.bridge/*)로 전달하십시오."
                .into(),
        );
        out
    }

    async fn list_tools(&self, _req: Option<PaginatedRequestParams>, ctx: RequestContext<RoleServer>) -> Result<ListToolsResult, McpError> {
        // 주체를 해석하지 못하면 도구를 보여주지 않습니다.
        let id = self.identity(&ctx).map_err(|e| McpError::invalid_request(e, None))?;

        let pol = self.gw.policy();
        let tools: Vec<Tool> = self
            .gw
            .registry()
            .specs()
            .into_iter()
            // **광고 축소일 뿐 보안 경계가 아닙니다.** RBAC·주체 스코프만 봅니다 —
            // ABAC 는 시각·네트워크·인자에 의존하므로 목록 시점에는 알 수 없습니다.
            // 실제 집행은 언제나 호출 시점의 evaluate 가 합니다.
            .filter(|spec| pol.visible(&id, spec))
            .map(|spec| {
                let mut t = Tool::new(
                    spec.name.clone(),
                    annotated_description(&spec),
                    Arc::new(to_json_object(&spec.input_schema)),
                )
                .annotate(annotations(&spec));
                t.output_schema = Some(Arc::new(to_json_object(&spec.output_schema)));
                t
            })
            .collect();

        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(&self, req: CallToolRequestParams, ctx: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        let tool = req.name.to_string();

        // 주체를 해석하지 못하면 익명으로 강등하지 않고 거절합니다.
        let id = match self.identity(&ctx) {
            | Ok(id) => id,
            | Err(e) => {
                return Ok(error_result("permission_denied", &e, ""));
            },
        };

        // rmcp 가 `_meta` 를 여기로 옮겨 놓습니다. params 쪽은 비어 있습니다.
        let (prompt, usage) = observability(Some(&ctx.meta));
        let args = req.arguments.unwrap_or_default();

        let res = match self
            .gw
            .handle(
                &id,
                Call {
                    tool: tool.clone(),
                    args,
                    prompt,
                    usage,
                },
            )
            .await
        {
            | Ok(r) => r,
            | Err(e) => return Ok(error_result(&e.code, &e.message, &e.fallback)),
        };

        // 승인 대기 — 실행하지 않고 요약만 돌려줍니다.
        if res.dry_run {
            let mut structured = Map::new();
            structured.insert("dry_run".into(), json!(true));
            structured.insert("approval_status".into(), json!(res.approval_status));
            structured.insert("approval_id".into(), json!(res.approval_id));
            structured.insert("summary".into(), json!(res.summary));
            structured.insert(FIELD_REQUEST_ID.into(), json!(res.request_id));

            let mut out = CallToolResult::success(vec![ContentBlock::text(res.summary.clone())]);
            out.structured_content = Some(Value::Object(structured));
            return Ok(out);
        }

        let payload = annotate(&res);
        let text = serde_json::to_string_pretty(&Value::Object(payload.clone())).unwrap_or_else(|_| "{}".into());

        // **텍스트와 구조화 결과 양쪽에 판단 필드를 실어야 합니다** — 모델이 실제로
        // 읽는 것은 텍스트이기 때문입니다.
        let mut out = CallToolResult::success(vec![ContentBlock::text(text)]);
        out.structured_content = Some(Value::Object(payload));
        Ok(out)
    }

    async fn list_resources(&self, _req: Option<PaginatedRequestParams>, ctx: RequestContext<RoleServer>) -> Result<ListResourcesResult, McpError> {
        self.identity(&ctx).map_err(|e| McpError::invalid_request(e, None))?;

        // 목록 자체는 주체와 무관합니다 — **접근 제어는 읽기에서** 합니다.
        let resources = self
            .resources
            .iter()
            .map(|r| {
                let mut m = rmcp::model::Resource::new(r.uri.clone(), r.name.clone());
                m.description = Some(r.description.clone());
                m.mime_type = Some(r.mime_type.clone());
                m
            })
            .collect();

        Ok(ListResourcesResult {
            resources,
            ..Default::default()
        })
    }

    async fn read_resource(&self, req: ReadResourceRequestParams, ctx: RequestContext<RoleServer>) -> Result<ReadResourceResult, McpError> {
        let id = self.identity(&ctx).map_err(|e| McpError::invalid_request(e, None))?;

        let Some(res) = self.resources.iter().find(|r| r.uri == req.uri) else {
            return Err(McpError::resource_not_found(req.uri.clone(), None));
        };

        // **자원 읽기도 도구와 동일한 행 수준 접근 제어를 통과합니다.**
        let body = res.read.read(&id).await.map_err(|e| McpError::invalid_request(e.to_string(), None))?;

        Ok(ReadResourceResult::new(vec![ResourceContents::TextResourceContents {
            uri: res.uri.clone(),
            mime_type: Some(res.mime_type.clone()),
            text: body,
            meta: None,
        }]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Sensitivity,
                          Spec};

    fn spec(name: &str, access: Access, risk: RiskLevel) -> Spec {
        Spec {
            name: name.into(),
            description: "설명".into(),
            system: "erp".into(),
            access,
            risk_level: risk,
            sensitivity: Sensitivity::Internal,
            ..Default::default()
        }
    }

    #[test]
    fn risk_level_maps_to_standard_annotations() {
        // L1 조회 — 읽기 전용, 비파괴.
        let a = annotations(&spec("get_invoice_status", Access::Read, RiskLevel::L1));
        assert_eq!(a.read_only_hint, Some(true));
        assert_eq!(a.destructive_hint, Some(false));
        assert_eq!(a.idempotent_hint, Some(true));
        assert_eq!(a.open_world_hint, Some(false));

        // L2 초안 — 쓰기지만 **비파괴**입니다. 초안은 효력이 없습니다.
        let a = annotations(&spec("draft_purchase_request", Access::Write, RiskLevel::L2));
        assert_eq!(a.read_only_hint, Some(false));
        assert_eq!(a.destructive_hint, Some(false));

        // L3 실행 — 파괴적. 클라이언트가 확인 UI 를 띄웁니다.
        let a = annotations(&spec("create_support_ticket", Access::Write, RiskLevel::L3));
        assert_eq!(a.destructive_hint, Some(true));

        // L4 자동 — 파괴적.
        let a = annotations(&spec("process_refund", Access::Write, RiskLevel::L4));
        assert_eq!(a.destructive_hint, Some(true));
    }

    #[test]
    fn description_carries_the_risk_metadata() {
        let d = annotated_description(&spec("process_refund", Access::Write, RiskLevel::L4));
        assert!(d.contains("시스템=erp"));
        assert!(d.contains("위험=L4"));
        assert!(d.contains("기본차단"));
        assert!(d.contains("승인필요"));

        let d = annotated_description(&spec("get_invoice_status", Access::Read, RiskLevel::L1));
        assert!(!d.contains("승인필요"));
    }

    #[test]
    fn meta_is_read_but_never_required() {
        // 관측은 통제가 아닙니다 — 없어도 호출은 정상 동작해야 합니다.
        let (prompt, usage) = observability(None);
        assert!(prompt.is_empty());
        assert_eq!(usage, Usage::default());

        let meta = rmcp::model::Meta(
            json!({
                META_PROMPT: "INV-1 결제됐어?",
                META_INPUT_TOKENS: 1500,
                META_OUTPUT_TOKENS: 200,
                META_COST_MICROS: 4200,
            })
            .as_object()
            .unwrap()
            .clone(),
        );
        let (prompt, usage) = observability(Some(&meta));
        assert_eq!(prompt, "INV-1 결제됐어?");
        assert_eq!(usage.input_tokens, 1500);
        assert_eq!(usage.cost_micros, 4200);
    }

    /// **회귀 검사.** rmcp 는 `_meta` 를 `RequestContext.meta` 로 옮기므로
    /// `CallToolRequestParams.meta` 는 언제나 비어 있습니다. 거기서 읽으면
    /// 프롬프트·토큰· 비용이 조용히 사라집니다 — 감사 로그와 비용 상한이
    /// 무력화됩니다.
    ///
    /// 이 사실은 단위 테스트로는 드러나지 않았고(Meta 를 직접 만들어
    /// 넘겼으므로), 실제 MCP 세션을 태워보고서야 잡혔습니다.
    #[test]
    fn call_tool_params_never_carry_meta_over_the_wire() {
        let wire = json!({
            "name": "get_invoice_status",
            "arguments": {"invoice_id": "INV-1"},
            "_meta": { META_PROMPT: "INV-1 결제됐어?", META_COST_MICROS: 4200 }
        });
        let params: CallToolRequestParams = serde_json::from_value(wire).unwrap();

        // 역직렬화 단계에서는 params 에 남아 있지만, rmcp 서비스 계층이 이것을
        // RequestContext.meta 로 옮깁니다. 그래서 핸들러는 **ctx.meta** 를 읽어야
        // 합니다.
        let from_params = observability(params.meta.as_ref());
        assert_eq!(from_params.0, "INV-1 결제됐어?");

        // 핸들러가 실제로 받는 경로(ctx.meta)에서도 같은 값이 나와야 합니다.
        let ctx_meta = rmcp::model::Meta(json!({ META_PROMPT: "INV-1 결제됐어?", META_COST_MICROS: 4200 }).as_object().unwrap().clone());
        let (prompt, usage) = observability(Some(&ctx_meta));
        assert_eq!(prompt, "INV-1 결제됐어?");
        assert_eq!(usage.cost_micros, 4200);
    }

    #[test]
    fn malformed_meta_is_ignored_not_rejected() {
        let meta = rmcp::model::Meta(json!({ META_INPUT_TOKENS: "not-a-number" }).as_object().unwrap().clone());
        let (_, usage) = observability(Some(&meta));
        assert_eq!(usage.input_tokens, 0);
    }

    #[test]
    fn gateway_errors_become_tool_results_not_protocol_errors() {
        // LLM 이 오류를 읽고 스스로 교정할 수 있어야 합니다.
        let r = error_result("permission_denied", "권한이 없습니다", "재무팀에 문의하세요");
        assert_eq!(r.is_error, Some(true));
        let s = r.structured_content.unwrap();
        assert_eq!(s["error_code"], json!("permission_denied"));
        assert_eq!(s["fallback"], json!("재무팀에 문의하세요"));
    }

    #[test]
    fn results_carry_the_judgment_fields() {
        let res = CallResult {
            tool: "get_customer_profile".into(),
            data: json!({"name":"김철수"}).as_object().unwrap().clone(),
            masked: true,
            narrowed: true,
            request_id: "req-abc".into(),
            ..Default::default()
        };
        let p = annotate(&res);
        assert_eq!(p[FIELD_MASKED], json!(true));
        assert_eq!(p[FIELD_NARROWED], json!(true));
        assert_eq!(p[FIELD_REQUEST_ID], json!("req-abc"));
        // 업무 데이터는 그대로입니다.
        assert_eq!(p["name"], json!("김철수"));
    }
}
