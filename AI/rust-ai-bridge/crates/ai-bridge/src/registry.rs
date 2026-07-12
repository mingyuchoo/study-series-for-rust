//! 도구 레지스트리 — 메타데이터 + 위험 등급 검증.
//!
//! **등록되지 않은 도구는 호출할 수 없습니다**(allowlist). 임의 SQL·API·쉘은
//! 애초에 표현할 방법이 없습니다.
//!
//! 레지스트리는 등록 시점에 명세를 검증합니다. 검증이 잡아내는 것들은 전부
//! "컴파일은 되지만 운영에서 사고가 나는" 조합입니다 — 등급을 잊은 도구,
//! 재시도하는 쓰기 도구, 스키마 없는 도구.

use crate::{auth::Identity,
            schema};
use anyhow::{Result,
             anyhow,
             bail};
use serde_json::{Map,
                 Value};
use std::{collections::HashMap,
          sync::{Arc,
                 RwLock},
          time::Duration};

/// 위험 등급.
///
/// 읽기/쓰기 2분법만으로는 "발주 초안 생성"(부작용 없음)과 "발주 실행"(되돌리기
/// 어려움)을 구분할 수 없습니다. **승인 관문은 읽기/쓰기가 아니라 등급에
/// 걸립니다.**
///
/// Go 의 `RiskUnspecified`(제로값) 에 대응하는 변형이 **없습니다.** Rust 에서는
/// 타입으로 강제할 수 있으므로, "등급을 잊은 도구가 가장 안전한 등급으로 조용히
/// 등록되는" 사고가 애초에 표현 불가능합니다. YAML 파싱 단계에서 없으면
/// 거부합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskLevel {
    /// 조회형 — 그대로 실행.
    L1,
    /// 작성 보조형 — 쓰기여도 승인 불필요 (초안은 효력이 없습니다).
    L2,
    /// 실행 보조형 — 승인 필요.
    L3,
    /// 자동 실행형 — 기본 차단 (`-allow-high-risk` 필요) + 승인 필요.
    L4,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            | RiskLevel::L1 => "L1(조회형)",
            | RiskLevel::L2 => "L2(작성 보조형)",
            | RiskLevel::L3 => "L3(실행 보조형)",
            | RiskLevel::L4 => "L4(자동 실행형)",
        };
        f.write_str(s)
    }
}

impl RiskLevel {
    /// `L1`~`L4` 또는 `1`~`4` 를 파싱합니다 (대소문자 무시).
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_uppercase().as_str() {
            | "L1" | "1" => Ok(RiskLevel::L1),
            | "L2" | "2" => Ok(RiskLevel::L2),
            | "L3" | "3" => Ok(RiskLevel::L3),
            | "L4" | "4" => Ok(RiskLevel::L4),
            | other => bail!("risk_level must be L1–L4, got {other:?}"),
        }
    }
}

/// 접근 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Access {
    #[default]
    Read,
    Write,
}

impl std::fmt::Display for Access {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            | Access::Read => "read",
            | Access::Write => "write",
        })
    }
}

impl Access {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_lowercase().as_str() {
            | "" | "read" => Ok(Access::Read),
            | "write" => Ok(Access::Write),
            | other => bail!("access must be read or write, got {other:?}"),
        }
    }
}

/// 데이터 민감도.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Sensitivity {
    Public,
    #[default]
    Internal,
    Confidential,
    Restricted,
}

impl std::fmt::Display for Sensitivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            | Sensitivity::Public => "public",
            | Sensitivity::Internal => "internal",
            | Sensitivity::Confidential => "confidential",
            | Sensitivity::Restricted => "restricted",
        })
    }
}

impl Sensitivity {
    /// 알 수 없는 값은 `internal` 로 봅니다 (Go 의 `parseSensitivity` 와 동일).
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            | "public" => Sensitivity::Public,
            | "confidential" => Sensitivity::Confidential,
            | "restricted" => Sensitivity::Restricted,
            | _ => Sensitivity::Internal,
        }
    }
}

/// 도구 핸들러.
///
/// **호출 주체를 함께 받습니다.** 정책 엔진은 "이 도구를 호출해도 되는가"까지만
/// 판단하고, 검색 결과에서 볼 수 없는 행을 걸러내는 일은 데이터를 아는 어댑터만
/// 할 수 있습니다.
#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    async fn call(&self, id: &Identity, args: &Map<String, Value>) -> Result<Value>;
}

pub type Handler = Arc<dyn ToolHandler>;

/// 클로저를 핸들러로 감싸는 헬퍼.
pub struct FnHandler<F>(pub F);

#[async_trait::async_trait]
impl<F, Fut> ToolHandler for FnHandler<F>
where
    F: Fn(Identity, Map<String, Value>) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<Value>> + Send,
{
    async fn call(&self, id: &Identity, args: &Map<String, Value>) -> Result<Value> { (self.0)(id.clone(), args.clone()).await }
}

/// 도구를 클로저로 만듭니다.
pub fn handler<F, Fut>(f: F) -> Handler
where
    F: Fn(Identity, Map<String, Value>) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<Value>> + Send + 'static,
{
    Arc::new(FnHandler(f))
}

/// 도구 명세.
#[derive(Debug, Clone)]
pub struct Spec {
    pub name: String,
    pub description: String,
    pub system: String,
    pub access: Access,
    pub risk_level: RiskLevel,
    pub sensitivity: Sensitivity,
    pub required_permissions: Vec<String>,
    pub approval_required: bool,
    /// 승인 유효 기간. **시계는 관리자가 결정한 시점부터 흐릅니다.**
    /// 0 이면 [`crate::approval::DEFAULT_TTL`](1시간)을 씁니다 — 무기한 승인은
    /// 없습니다.
    pub approval_ttl: Duration,
    pub rate_limit_per_min: i64,
    pub timeout_ms: i64,
    /// 쓰기 도구는 0 이어야 합니다 — 재시도하면 티켓이 두 번 생성됩니다.
    pub max_retries: i64,
    pub log_retention_days: i64,
    pub mask_fields: Vec<String>,
    /// 권한이 없거나 도구가 실패했을 때 사용자에게 안내할 대안.
    pub fallback: String,
    pub input_schema: Value,
    pub output_schema: Value,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            system: String::new(),
            access: Access::Read,
            risk_level: RiskLevel::L1,
            sensitivity: Sensitivity::Internal,
            required_permissions: Vec::new(),
            approval_required: false,
            approval_ttl: Duration::ZERO,
            rate_limit_per_min: 0,
            timeout_ms: 0,
            max_retries: 0,
            log_retention_days: 0,
            mask_fields: Vec::new(),
            fallback: String::new(),
            input_schema: Value::Null,
            output_schema: Value::Null,
        }
    }
}

/// 명세 + 핸들러.
#[derive(Clone)]
pub struct Tool {
    pub spec: Spec,
    pub handler: Handler,
}

impl std::fmt::Debug for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("Tool").field("spec", &self.spec).finish() }
}

/// 도구 레지스트리 (동시 접근 안전, 핫 리로드 가능).
#[derive(Default)]
pub struct Registry {
    tools: RwLock<HashMap<String, Tool>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("Registry").field("len", &self.len()).finish() }
}

impl Registry {
    pub fn new() -> Self { Self::default() }

    /// 도구를 등록합니다. 이미 있으면 거부합니다.
    pub fn register(&self, t: Tool) -> Result<()> {
        if t.spec.name.is_empty() {
            bail!("register tool: name is required");
        }
        validate_spec(&t.spec).map_err(|e| anyhow!("register tool {:?}: {e}", t.spec.name))?;
        let mut tools = self.tools.write().unwrap();
        if tools.contains_key(&t.spec.name) {
            bail!("register tool {:?}: already registered", t.spec.name);
        }
        tools.insert(t.spec.name.clone(), t);
        Ok(())
    }

    /// 도구를 등록하거나 **덮어씁니다**. 동적 카탈로그 리로드와 핸들러 재배선이
    /// 씁니다.
    pub fn replace(&self, t: Tool) -> Result<()> {
        if t.spec.name.is_empty() {
            bail!("register tool: name is required");
        }
        validate_spec(&t.spec).map_err(|e| anyhow!("register tool {:?}: {e}", t.spec.name))?;
        self.tools.write().unwrap().insert(t.spec.name.clone(), t);
        Ok(())
    }

    pub fn unregister(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            bail!("unregister tool: name is required");
        }
        if self.tools.write().unwrap().remove(name).is_none() {
            bail!("unregister tool {name:?}: not registered");
        }
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Option<Tool> { self.tools.read().unwrap().get(name).cloned() }

    /// 모든 명세를 이름순으로 돌려줍니다.
    pub fn specs(&self) -> Vec<Spec> {
        let tools = self.tools.read().unwrap();
        let mut out: Vec<Spec> = tools.values().map(|t| t.spec.clone()).collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn names(&self) -> Vec<String> {
        let mut out: Vec<String> = self.tools.read().unwrap().keys().cloned().collect();
        out.sort();
        out
    }

    pub fn len(&self) -> usize { self.tools.read().unwrap().len() }

    pub fn is_empty(&self) -> bool { self.len() == 0 }
}

/// 명세 검증. **여기서 거부되는 조합이 곧 게이트웨이의 안전 불변식입니다.**
fn validate_spec(s: &Spec) -> Result<()> {
    if s.description.is_empty() {
        bail!("Description is required");
    }
    if s.system.is_empty() {
        bail!("System is required");
    }
    if s.required_permissions.is_empty() {
        bail!("RequiredPermissions must contain at least one permission");
    }
    if s.rate_limit_per_min <= 0 {
        bail!("RateLimitPerMin must be greater than zero");
    }
    if s.log_retention_days <= 0 {
        bail!("LogRetentionDays must be greater than zero");
    }
    if s.fallback.is_empty() {
        bail!("Fallback is required");
    }
    if s.input_schema.is_null() {
        bail!("InputSchema is required (없으면 입력 검증이 생략됩니다)");
    }
    if s.output_schema.is_null() {
        bail!("OutputSchema is required (없으면 출력 검증이 생략됩니다)");
    }
    schema::compile(&s.input_schema).map_err(|e| anyhow!("InputSchema: {e}"))?;
    schema::compile(&s.output_schema).map_err(|e| anyhow!("OutputSchema: {e}"))?;

    // 쓰기 도구는 L1 일 수 없습니다 — 부작용이 있는데 조회형으로 선언된 것.
    if s.access == Access::Write && s.risk_level < RiskLevel::L2 {
        bail!("쓰기 도구는 RiskLevel이 L2 이상이어야 합니다(현재 {})", s.risk_level);
    }
    // 읽기 도구는 L3/L4 일 수 없습니다 — 조회에 승인 관문을 다는 것은 등급
    // 오용입니다.
    if s.access == Access::Read && s.risk_level > RiskLevel::L2 {
        bail!("읽기 도구에 {} 등급은 맞지 않습니다", s.risk_level);
    }
    // 쓰기 재시도는 중복 실행입니다. 티켓이 두 번 생성됩니다.
    if s.access == Access::Write && s.max_retries > 0 {
        bail!("쓰기 도구는 재시도할 수 없습니다(중복 실행 위험). MaxRetries={}", s.max_retries);
    }
    if s.max_retries < 0 || s.timeout_ms < 0 {
        bail!("MaxRetries와 TimeoutMS는 음수일 수 없습니다");
    }
    // 승인 대상이 될 수 없는 도구의 TTL 은 의미가 없습니다(설정 오류를 조용히
    // 넘기지 않음).
    if !s.approval_ttl.is_zero() && s.access == Access::Read && s.risk_level == RiskLevel::L1 && !s.approval_required {
        bail!("ApprovalTTL은 승인 대상이 될 수 있는 도구에만 의미가 있습니다");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj() -> Value { json!({"type": "object", "properties": {}, "additionalProperties": false}) }

    fn ok_spec() -> Spec {
        Spec {
            name: "get_invoice_status".into(),
            description: "송장 상태 조회".into(),
            system: "erp".into(),
            access: Access::Read,
            risk_level: RiskLevel::L1,
            required_permissions: vec!["erp.invoice.read".into()],
            rate_limit_per_min: 60,
            log_retention_days: 90,
            fallback: "재무팀에 문의하세요".into(),
            input_schema: obj(),
            output_schema: obj(),
            ..Default::default()
        }
    }

    fn tool(spec: Spec) -> Tool {
        Tool {
            spec,
            handler: handler(|_id, _args| async { Ok(json!({})) }),
        }
    }

    #[test]
    fn registers_a_valid_tool() {
        let r = Registry::new();
        assert!(r.register(tool(ok_spec())).is_ok());
        assert_eq!(r.len(), 1);
        assert!(r.lookup("get_invoice_status").is_some());
    }

    #[test]
    fn rejects_duplicate_registration() {
        let r = Registry::new();
        r.register(tool(ok_spec())).unwrap();
        assert!(r.register(tool(ok_spec())).is_err());
    }

    #[test]
    fn replace_upserts_without_duplicate_error() {
        let r = Registry::new();
        r.register(tool(ok_spec())).unwrap();
        assert!(r.replace(tool(ok_spec())).is_ok());
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn rejects_write_tool_at_l1() {
        // 부작용이 있는데 조회형으로 선언된 도구.
        let mut s = ok_spec();
        s.access = Access::Write;
        s.risk_level = RiskLevel::L1;
        assert!(Registry::new().register(tool(s)).is_err());
    }

    #[test]
    fn rejects_read_tool_above_l2() {
        let mut s = ok_spec();
        s.access = Access::Read;
        s.risk_level = RiskLevel::L3;
        assert!(Registry::new().register(tool(s)).is_err());
    }

    #[test]
    fn rejects_write_tool_with_retries() {
        // 재시도하는 쓰기 도구는 티켓을 두 번 만듭니다.
        let mut s = ok_spec();
        s.access = Access::Write;
        s.risk_level = RiskLevel::L3;
        s.max_retries = 2;
        assert!(Registry::new().register(tool(s)).is_err());
    }

    #[test]
    fn allows_write_tool_at_l3_without_retries() {
        let mut s = ok_spec();
        s.name = "create_support_ticket".into();
        s.access = Access::Write;
        s.risk_level = RiskLevel::L3;
        s.max_retries = 0;
        assert!(Registry::new().register(tool(s)).is_ok());
    }

    #[test]
    fn rejects_missing_schemas() {
        let mut s = ok_spec();
        s.input_schema = Value::Null;
        assert!(Registry::new().register(tool(s)).is_err());

        let mut s = ok_spec();
        s.output_schema = Value::Null;
        assert!(Registry::new().register(tool(s)).is_err());
    }

    #[test]
    fn rejects_missing_permissions_and_fallback() {
        let mut s = ok_spec();
        s.required_permissions = vec![];
        assert!(Registry::new().register(tool(s)).is_err());

        let mut s = ok_spec();
        s.fallback = String::new();
        assert!(Registry::new().register(tool(s)).is_err());
    }

    #[test]
    fn rejects_nonpositive_rate_limit_and_retention() {
        let mut s = ok_spec();
        s.rate_limit_per_min = 0;
        assert!(Registry::new().register(tool(s)).is_err());

        let mut s = ok_spec();
        s.log_retention_days = 0;
        assert!(Registry::new().register(tool(s)).is_err());
    }

    #[test]
    fn rejects_approval_ttl_on_plain_l1_read_tool() {
        let mut s = ok_spec();
        s.approval_ttl = Duration::from_secs(3600);
        assert!(Registry::new().register(tool(s)).is_err());
    }

    #[test]
    fn specs_are_sorted_by_name() {
        let r = Registry::new();
        let mut b = ok_spec();
        b.name = "b_tool".into();
        let mut a = ok_spec();
        a.name = "a_tool".into();
        r.register(tool(b)).unwrap();
        r.register(tool(a)).unwrap();
        assert_eq!(r.specs().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["a_tool", "b_tool"]);
    }

    #[test]
    fn risk_level_parses_both_forms() {
        assert_eq!(RiskLevel::parse("L3").unwrap(), RiskLevel::L3);
        assert_eq!(RiskLevel::parse("3").unwrap(), RiskLevel::L3);
        assert_eq!(RiskLevel::parse("l4").unwrap(), RiskLevel::L4);
        assert!(RiskLevel::parse("L5").is_err());
    }

    #[test]
    fn risk_levels_are_ordered() {
        assert!(RiskLevel::L4 > RiskLevel::L3);
        assert!(RiskLevel::L3 >= RiskLevel::L3);
        assert!(RiskLevel::L2 < RiskLevel::L3);
    }
}
