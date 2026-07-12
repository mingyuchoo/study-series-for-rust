//! 일시적 장애와 업무 오류의 구분.
//!
//! "지금 안 되는 것"과 "원래 없는 것"은 다릅니다. 레거시가 느리거나 죽은 것은
//! 재시도할 가치가 있지만, "존재하지 않는 송장"은 백 번 물어도 없습니다. 이
//! 구분이 게이트웨이의 재시도와 서킷 브레이커를 좌우합니다 — 업무 오류를
//! 브레이커에 먹이면 없는 송장을 몇 번 조회했다는 이유로 ERP 회로가 열립니다.

use std::fmt;

/// "이건 일시적 장애다"라고 명시적으로 표시하는 오류 래퍼.
///
/// 어댑터가 5xx·연결 실패 같은 것을 이 타입으로 감싸면 게이트웨이가
/// 재시도합니다.
#[derive(Debug)]
pub struct TemporaryError(pub anyhow::Error);

impl fmt::Display for TemporaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "temporary: {}", self.0) }
}

impl std::error::Error for TemporaryError {
    /// 감싼 오류 **자신**을 내놓아야 합니다. `self.0.source()` 를 돌려주면 감싼
    /// 오류를 건너뛰어, `temporary(Canceled)` 의 체인에서 `Canceled` 가
    /// 사라집니다 — 그러면 "취소는 재시도하지 않는다"는 규칙이 조용히
    /// 깨집니다.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(self.0.as_ref()) }
}

/// 호출자가 취소했음을 나타내는 표식. Go 의 `context.Canceled` 대응.
#[derive(Debug, Clone, Copy)]
pub struct Canceled;

impl fmt::Display for Canceled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "context canceled") }
}

impl std::error::Error for Canceled {}

/// 시간 초과. Go 의 `context.DeadlineExceeded` 대응.
#[derive(Debug, Clone, Copy)]
pub struct DeadlineExceeded;

impl fmt::Display for DeadlineExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "context deadline exceeded") }
}

impl std::error::Error for DeadlineExceeded {}

/// 오류를 일시적 장애로 표시합니다.
pub fn temporary(err: impl Into<anyhow::Error>) -> anyhow::Error { anyhow::Error::new(TemporaryError(err.into())) }

/// 형식 문자열로 일시적 장애를 만듭니다.
#[macro_export]
macro_rules! temporaryf {
    ($($arg:tt)*) => {
        $crate::transient::temporary(::anyhow::anyhow!($($arg)*))
    };
}

/// 재시도할 가치가 있는 오류인지 판정합니다.
///
/// 검사 순서가 중요합니다:
///
/// 1. **취소가 가장 먼저** — 호출자가 이미 포기했는데 재시도하는 것은 의미가
///    없습니다. 다른 어떤 표식이 함께 붙어 있어도 취소가 이깁니다.
/// 2. 시간 초과 — 상대가 느릴 뿐이므로 재시도합니다.
/// 3. [`TemporaryError`] 표식 — 어댑터가 명시적으로 일시적이라고 선언한 것.
/// 4. 소켓 수준 타임아웃 (`reqwest` 의 timeout, `io::ErrorKind::TimedOut`).
/// 5. 그 외는 전부 업무 오류로 봅니다 — **재시도하지 않고 브레이커에도 먹이지
///    않습니다.**
pub fn is_temporary(err: &anyhow::Error) -> bool {
    // 1. 취소는 무조건 재시도 금지 — 체인 어디에 있든 즉시 false.
    if err.chain().any(|e| e.is::<Canceled>()) {
        return false;
    }
    for cause in err.chain() {
        // 2. 시간 초과.
        if cause.is::<DeadlineExceeded>() || cause.is::<tokio::time::error::Elapsed>() {
            return true;
        }
        // 3. 명시적 표식.
        if cause.is::<TemporaryError>() {
            return true;
        }
        // 4. 소켓 수준 타임아웃.
        if let Some(io) = cause.downcast_ref::<std::io::Error>()
            && matches!(io.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::ConnectionReset)
        {
            return true;
        }
        if let Some(re) = cause.downcast_ref::<reqwest::Error>()
            && (re.is_timeout() || re.is_connect())
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn business_errors_are_not_temporary() {
        // "송장이 없다"는 재시도해도 결과가 같습니다.
        let err = anyhow::anyhow!("invoice INV-9999 not found");
        assert!(!is_temporary(&err));
    }

    #[test]
    fn explicit_marker_is_temporary() {
        let err = temporary(anyhow::anyhow!("ERP returned 503"));
        assert!(is_temporary(&err));
    }

    #[test]
    fn deadline_is_temporary_but_cancel_is_not() {
        assert!(is_temporary(&anyhow::Error::new(DeadlineExceeded)));
        assert!(!is_temporary(&anyhow::Error::new(Canceled)));
    }

    #[test]
    fn cancel_wins_over_temporary_marker() {
        // 호출자가 포기한 뒤에는 "일시적"이라는 표식이 붙어 있어도 재시도하지 않습니다.
        let err = temporary(anyhow::Error::new(Canceled));
        assert!(!is_temporary(&err));
    }

    #[test]
    fn wrapped_marker_is_found_through_the_chain() {
        let err = temporary(anyhow::anyhow!("boom")).context("calling erp");
        assert!(is_temporary(&err));
    }
}
