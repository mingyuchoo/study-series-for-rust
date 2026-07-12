//! 게이트웨이 파이프라인 — **집행 계층**.
//!
//! 모든 도구 호출이 여기를 통과합니다.
//!
//! ```text
//! admit (사전 통제)
//!   A. 도구 조회 (allowlist)
//!   B. 레이트 리밋
//!   C. 비용 상한
//!   D. 입력 검증 (JSON Schema)
//!   E. 정책 판단 (RBAC + 주체 스코프 + ABAC)
//!   F. 고위험(L4) 차단
//!   G. 승인 관문 (미승인이면 dry-run)
//!   H. L3+ 사전 감사 (fail-closed)
//! execute (어댑터)
//!   I. 서킷 브레이커 · 타임아웃 · 재시도
//! shape_output (사후 처리)
//!   J. 출력 검증
//!   K. 의무 집행
//!   L. PII 마스킹
//!   M. 간접 인젝션 탐지
//!   N. 감사 로그
//! ```
//!
//! # 반드시 지켜야 할 것
//!
//! - **모든 종료 분기가 감사됩니다.** 거부·오류·dry-run 전부.
//! - **거부된 호출도 LLM 비용을 누적합니다** — 이미 발생한 비용이기 때문입니다.
//! - **H 단계만 감사 실패를 치명적으로 봅니다.** L3+ 실행 직전에 감사를 보장할
//!   수 없으면 실행하지 않습니다.
//! - **어댑터 실패는 `denied` 가 아니라 `allowed` 로 감사됩니다** — 인가는
//!   통과했고 시스템이 아픈 것이므로. 이 구분이 메트릭의 `decision` 레이블을
//!   의미 있게 만듭니다.

mod approver;
mod execute;
mod obligations;
mod output;
mod pipeline;

use crate::{audit,
            breaker::Breaker,
            budget,
            injection::Detector,
            inventory::Inventory,
            pii::Masker,
            policy,
            ratelimit,
            registry::Registry,
            telemetry::Telemetry};
pub use approver::{Approval,
                   Approver,
                   DenyApprover,
                   StoreApprover};
use serde_json::{Map,
                 Value};
use std::{sync::Arc,
          time::Duration};

/// LLM 턴의 사용량. 오케스트레이터가 MCP `_meta` 로 실어 보냅니다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// 비용(마이크로). 부동소수 오차를 피하려고 정수입니다.
    pub cost_micros: i64,
}

/// 도구 호출 하나.
#[derive(Debug, Clone, Default)]
pub struct Call {
    pub tool: String,
    pub args: Map<String, Value>,
    /// 이 호출을 유발한 사용자 발화. 감사와 인젝션 탐지에 쓰입니다.
    pub prompt: String,
    pub usage: Usage,
}

/// 도구 호출 결과.
#[derive(Debug, Clone, Default)]
pub struct CallResult {
    pub tool: String,
    /// 마스킹·의무가 적용된 출력.
    pub data: Map<String, Value>,
    pub masked: bool,
    /// 의무가 필드를 지웠거나 행을 잘랐음.
    pub narrowed: bool,
    pub dry_run: bool,
    /// `n/a` | `pending` | `approved` | `rejected`.
    pub approval_status: String,
    pub approval_id: String,
    /// dry-run 일 때만.
    pub summary: String,
    pub request_id: String,
}

/// 게이트웨이가 거부하거나 실패한 이유.
///
/// **오류 코드가 "지금 안 되는 것"과 "원래 없는 것"을 구분합니다.** LLM 이
/// 재시도할지 포기할지 판단하는 근거입니다.
#[derive(Debug, Clone, thiserror::Error)]
#[error("[{code}] {message}{}", fallback_suffix(.fallback))]
pub struct Error {
    pub code: String,
    pub message: String,
    /// 대안 안내. "담당 부서에 문의하세요"가 아니라 구체적인 경로여야 합니다.
    pub fallback: String,
}

fn fallback_suffix(f: &str) -> String { if f.is_empty() { String::new() } else { format!(" (대안: {f})") } }

impl Error {
    pub fn new(code: &str, message: impl Into<String>, fallback: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            fallback: fallback.into(),
        }
    }
}

/// **자기 자신이 초래한 거부** — 정책이 제대로 동작한 것입니다.
///
/// `timeout`·`unavailable`·`adapter_error` 는 여기 **없습니다** — 그것들은
/// "시스템이 아픈 것"이므로 `decision=allowed` 로 계측됩니다. 이 구분이
/// 대시보드에서 "정책이 막았다"와 "레거시가 죽었다"를 가릅니다.
pub const DENIAL_CODES: &[&str] = &[
    "not_found",
    "invalid_input",
    "permission_denied",
    "rate_limited",
    "budget_exceeded",
    "high_risk_blocked",
    "approval_rejected",
    "approval_error",
];

pub fn is_denial(code: &str) -> bool { DENIAL_CODES.contains(&code) }

const DEFAULT_RATE_LIMIT_PER_MIN: i64 = 60;
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// 게이트웨이가 필요로 하는 것들.
///
/// **필요한 만큼만 요구합니다.** `audit` 는 [`audit::Recorder`](쓰기
/// 전용)입니다 — 파이프라인이 감사 기록을 조회하거나 지울 수 있게 두지
/// 않습니다.
pub struct Deps {
    pub registry: Arc<Registry>,
    pub policy: Arc<policy::Engine>,
    pub audit: Arc<dyn audit::Recorder>,
    /// 없으면 기본 마스커.
    pub masker: Option<Arc<Masker>>,
    /// 없으면 프로세스 메모리 카운터.
    pub limiter: Option<Arc<dyn ratelimit::Limiter>>,
    /// 없으면 [`DenyApprover`] — **fail-closed** 입니다.
    pub approver: Option<Arc<dyn Approver>>,
    pub breaker: Option<Arc<Breaker>>,
    pub inventory: Option<Arc<Inventory>>,
    /// 없으면 상한 없음.
    pub budget: Option<Arc<dyn budget::Tracker>>,
    pub telemetry: Option<Arc<Telemetry>>,
    /// 없으면 기본 규칙으로 **항상 켜집니다.**
    pub injection: Option<Arc<Detector>>,
    /// **L4 도구는 이것이 명시적으로 참일 때만 실행됩니다.**
    pub allow_high_risk: bool,
}

/// 집행 계층.
pub struct Gateway {
    pub(crate) registry: Arc<Registry>,
    pub(crate) policy: Arc<policy::Engine>,
    pub(crate) audit: Arc<dyn audit::Recorder>,
    pub(crate) masker: Arc<Masker>,
    pub(crate) limiter: Arc<dyn ratelimit::Limiter>,
    pub(crate) approver: Arc<dyn Approver>,
    pub(crate) breaker: Arc<Breaker>,
    pub(crate) inventory: Option<Arc<Inventory>>,
    pub(crate) budget: Arc<dyn budget::Tracker>,
    pub(crate) telemetry: Option<Arc<Telemetry>>,
    pub(crate) injection: Arc<Detector>,
    pub(crate) allow_high_risk: bool,
    pub(crate) clock: crate::clock::SharedClock,
}

impl std::fmt::Debug for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gateway")
            .field("tools", &self.registry.len())
            .field("allow_high_risk", &self.allow_high_risk)
            .finish()
    }
}

impl Gateway {
    pub fn new(d: Deps) -> Self {
        Self {
            registry: d.registry,
            policy: d.policy,
            audit: d.audit,
            masker: d.masker.unwrap_or_else(|| Arc::new(Masker::new())),
            limiter: d.limiter.unwrap_or_else(|| Arc::new(ratelimit::Memory::new())),
            // **기본이 거부입니다.** 승인자를 꽂지 않으면 L3+ 는 실행되지 않습니다.
            approver: d.approver.unwrap_or_else(|| Arc::new(DenyApprover)),
            breaker: d.breaker.unwrap_or_default(),
            inventory: d.inventory,
            budget: d.budget.unwrap_or_else(|| Arc::new(budget::Memory::new(0))),
            telemetry: d.telemetry,
            // 지정하지 않으면 기본 규칙으로 **항상 켜집니다.**
            injection: d.injection.unwrap_or_else(|| Arc::new(Detector::new())),
            allow_high_risk: d.allow_high_risk,
            clock: crate::clock::system(),
        }
    }

    pub fn with_clock(mut self, clock: crate::clock::SharedClock) -> Self {
        self.clock = clock;
        self
    }

    pub fn registry(&self) -> &Arc<Registry> { &self.registry }

    pub fn policy(&self) -> &Arc<policy::Engine> { &self.policy }

    pub fn breaker_statuses(&self) -> Vec<crate::breaker::Status> { self.breaker.statuses() }

    pub async fn budget_snapshot(&self) -> Vec<budget::Entry> { self.budget.snapshot().await }

    /// 권한이 없을 때 **어디에 요청해야 하는지** 안내합니다.
    pub(crate) fn access_request_path(&self, system: &str) -> String {
        self.inventory
            .as_ref()
            .map(|i| i.access_request_path(system))
            .unwrap_or_else(|| "담당 부서에 접근 요청을 제출하세요".to_string())
    }

    pub(crate) fn rate_limit_for(&self, limit: i64) -> i64 { if limit == 0 { DEFAULT_RATE_LIMIT_PER_MIN } else { limit } }
}

pub(crate) fn new_request_id() -> String {
    use rand::Rng as _;
    let mut b = [0u8; 6];
    rand::rng().fill_bytes(&mut b);
    format!("req-{}", hex::encode(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denial_codes_exclude_system_failures() {
        // 이 구분이 "정책이 막았다"와 "레거시가 죽었다"를 가릅니다.
        assert!(is_denial("permission_denied"));
        assert!(is_denial("budget_exceeded"));
        assert!(!is_denial("timeout"));
        assert!(!is_denial("unavailable"));
        assert!(!is_denial("adapter_error"));
    }

    #[test]
    fn error_display_includes_the_fallback() {
        let e = Error::new("permission_denied", "권한이 없습니다", "재무팀에 문의하세요");
        assert_eq!(e.to_string(), "[permission_denied] 권한이 없습니다 (대안: 재무팀에 문의하세요)");

        let e = Error::new("timeout", "지연", "");
        assert_eq!(e.to_string(), "[timeout] 지연");
    }
}
