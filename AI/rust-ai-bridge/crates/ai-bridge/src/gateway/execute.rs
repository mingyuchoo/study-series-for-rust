//! 어댑터 실행 — 서킷 브레이커 · 타임아웃 · 재시도.
//!
//! 레거시가 느리거나 죽어도 게이트웨이가 함께 멈추지 않습니다.
//!
//! **재시도는 두 조건을 모두 만족할 때만 합니다** — **읽기 도구**이고 **일시적
//! 장애**일 때. 쓰기를 재시도하면 티켓이 두 번 생성되므로, 레지스트리가
//! `max_retries > 0` 인 쓰기 도구를 등록 시점에 거부합니다.

use super::{DEFAULT_TIMEOUT,
            Error,
            Gateway};
use crate::{auth::Identity,
            breaker::ErrOpen,
            registry::{Access,
                       Spec,
                       Tool},
            transient};
use anyhow::anyhow;
use serde_json::Value;
use std::time::Duration;

/// 지수 백오프: 50ms · 100ms · 200ms …
fn retry_backoff(attempt: u32) -> Duration { Duration::from_millis(50) * 2u32.pow(attempt) }

impl Gateway {
    pub(crate) async fn execute(&self, tool: &Tool, id: &Identity, args: &serde_json::Map<String, Value>) -> Result<Value, anyhow::Error> {
        let spec = &tool.spec;

        // 회로가 열려 있으면 레거시를 아예 부르지 않고 즉시 실패시킵니다(fail fast).
        // 죽어가는 시스템에 부하를 더 얹지 않기 위함입니다.
        if let Err(open) = self.breaker.allow(&spec.system) {
            return Err(anyhow::Error::new(open));
        }

        let timeout = if spec.timeout_ms > 0 {
            Duration::from_millis(spec.timeout_ms as u64)
        } else {
            DEFAULT_TIMEOUT
        };

        // **쓰기는 절대 재시도하지 않습니다.**
        let attempts = if spec.access == Access::Read && spec.max_retries > 0 {
            1 + spec.max_retries as u32
        } else {
            1
        };

        let mut last: Option<anyhow::Error> = None;
        for attempt in 0 .. attempts {
            if attempt > 0 {
                tokio::time::sleep(retry_backoff(attempt - 1)).await;
            }

            let call = tool.handler.call(id, args);
            let result = match tokio::time::timeout(timeout, call).await {
                | Ok(r) => r,
                | Err(_) => Err(anyhow::Error::new(transient::DeadlineExceeded)),
            };

            match result {
                | Ok(out) => {
                    self.breaker.success(&spec.system);
                    return Ok(out);
                },
                | Err(err) => {
                    // 업무 오류는 재시도해도 결과가 같고, **브레이커에도 먹이지 않습니다** —
                    // 없는 송장을 여러 번 조회했다고 ERP 가 죽은 것은 아닙니다.
                    if !transient::is_temporary(&err) {
                        return Err(err);
                    }
                    self.breaker.failure(&spec.system, &err.to_string());
                    last = Some(err);
                },
            }
        }

        Err(anyhow!("{attempts}회 시도 후 실패: {}", last.map(|e| e.to_string()).unwrap_or_default()))
    }
}

/// 어댑터 오류를 게이트웨이 오류 코드로 옮깁니다.
///
/// **"지금 안 되는 것"과 "원래 없는 것"을 구분합니다** — LLM 이 재시도할지
/// 포기할지 판단하는 근거입니다.
pub(crate) fn adapter_error(err: &anyhow::Error, spec: &Spec) -> Error {
    // 회로 개방.
    if let Some(open) = err.chain().find_map(|e| e.downcast_ref::<ErrOpen>()) {
        return Error::new(
            "unavailable",
            format!(
                "{} 시스템이 일시적으로 응답하지 않아 호출을 차단했습니다({}초 후 재시도).",
                spec.system,
                open.retry_in.as_secs()
            ),
            &spec.fallback,
        );
    }
    // 시간 초과.
    if err
        .chain()
        .any(|e| e.is::<transient::DeadlineExceeded>() || e.is::<tokio::time::error::Elapsed>())
    {
        return Error::new(
            "timeout",
            format!("{} 시스템 응답이 지연되어 호출을 중단했습니다.", spec.system),
            &spec.fallback,
        );
    }
    // 그 밖의 일시적 장애.
    if transient::is_temporary(err) {
        return Error::new("unavailable", format!("{} 시스템 호출에 실패했습니다: {err}", spec.system), &spec.fallback);
    }
    // 업무 오류 — 재시도해도 같습니다.
    Error::new("adapter_error", err.to_string(), &spec.fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Spec;

    fn spec() -> Spec {
        Spec {
            system: "erp".into(),
            fallback: "재무팀에 문의하세요".into(),
            ..Default::default()
        }
    }

    #[test]
    fn business_errors_map_to_adapter_error() {
        let e = adapter_error(&anyhow!("송장 INV-9999 을(를) 찾을 수 없습니다"), &spec());
        assert_eq!(e.code, "adapter_error");
        assert_eq!(e.fallback, "재무팀에 문의하세요");
    }

    #[test]
    fn deadline_maps_to_timeout() {
        let e = adapter_error(&anyhow::Error::new(transient::DeadlineExceeded), &spec());
        assert_eq!(e.code, "timeout");
    }

    #[test]
    fn transient_failures_map_to_unavailable() {
        let e = adapter_error(&transient::temporary(anyhow!("ERP 503")), &spec());
        assert_eq!(e.code, "unavailable");
    }

    #[test]
    fn an_open_circuit_maps_to_unavailable_with_retry_hint() {
        let open = ErrOpen {
            key: "erp".into(),
            retry_in: Duration::from_secs(25),
            failures: 5,
        };
        let e = adapter_error(&anyhow::Error::new(open), &spec());
        assert_eq!(e.code, "unavailable");
        assert!(e.message.contains("25초 후 재시도"));
    }

    #[test]
    fn backoff_grows_exponentially() {
        assert_eq!(retry_backoff(0), Duration::from_millis(50));
        assert_eq!(retry_backoff(1), Duration::from_millis(100));
        assert_eq!(retry_backoff(2), Duration::from_millis(200));
    }
}
