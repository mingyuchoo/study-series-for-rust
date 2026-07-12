//! 주입 가능한 시계.
//!
//! 승인 TTL 만료, 업무시간 판정, 워크플로 lease 는 모두 "지금이 몇 시인가"에
//! 의존합니다. 실제 시계를 그대로 쓰면 그 규칙들을 테스트할 수 없으므로(만료를
//! 기다릴 수는 없습니다), 시계를 값으로 주입합니다. Go 의 `now func()
//! time.Time` 필드에 대응합니다.

use chrono::{DateTime,
             TimeZone,
             Utc};
use std::sync::{Arc,
                atomic::{AtomicI64,
                         Ordering}};

/// 현재 시각을 알려주는 것.
pub trait Clock: Send + Sync + std::fmt::Debug {
    fn now(&self) -> DateTime<Utc>;
}

/// 실제 벽시계.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> { Utc::now() }
}

/// 테스트용 고정·전진 가능 시계.
///
/// "승인 후 3시간이 지나면 만료된다" 같은 규칙은 시간을 앞으로 감아야만 검사할
/// 수 있습니다.
#[derive(Debug)]
pub struct TestClock(AtomicI64);

impl TestClock {
    /// 주어진 시각에 멈춘 시계를 만듭니다.
    pub fn new(at: DateTime<Utc>) -> Self { Self(AtomicI64::new(at.timestamp_nanos_opt().unwrap_or(0))) }

    /// 적합성 스위트가 쓰는 기준 시각 (2026-07-10T09:00:00Z).
    pub fn epoch() -> Self { Self::new(Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap()) }

    /// 시계를 앞으로 감습니다.
    pub fn advance(&self, d: chrono::Duration) { self.0.fetch_add(d.num_nanoseconds().unwrap_or(0), Ordering::SeqCst); }
}

impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> { Utc.timestamp_nanos(self.0.load(Ordering::SeqCst)) }
}

/// 공유 시계 핸들.
pub type SharedClock = Arc<dyn Clock>;

/// 기본 시계(실제 벽시계) 핸들을 만듭니다.
pub fn system() -> SharedClock { Arc::new(SystemClock) }

/// Go 의 `time.Time.Format(time.RFC3339)` 과 같은 문자열을 만듭니다.
///
/// 감사·승인·eval 테이블은 **초 정밀도** RFC3339 로 저장합니다. 해시 입력도
/// 같은 값으로 정규화되므로 이 함수를 거치지 않으면 해시 체인이 재현되지
/// 않습니다.
pub fn to_rfc3339(t: DateTime<Utc>) -> String { t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true) }

/// `workflow_event.ts` 와 lease 메타데이터가 쓰는 나노초 정밀도 표현.
pub fn to_rfc3339_nanos(t: DateTime<Utc>) -> String { t.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true) }

/// RFC3339 문자열을 파싱합니다. 실패하면 `None`.
pub fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> { DateTime::parse_from_rfc3339(s).ok().map(|t| t.with_timezone(&Utc)) }

/// 초 정밀도로 절삭합니다.
///
/// Go 는 저장 직전 `time.Parse(RFC3339, t.Format(RFC3339))` 으로 왕복시켜 하위
/// 정밀도를 버립니다. 저장 형식과 해시 입력을 같게 만들기 위한 것이므로 그대로
/// 따릅니다.
pub fn truncate_to_second(t: DateTime<Utc>) -> DateTime<Utc> { parse_rfc3339(&to_rfc3339(t)).unwrap_or(t) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_advances() {
        let c = TestClock::epoch();
        let t0 = c.now();
        c.advance(chrono::Duration::hours(3));
        assert_eq!(c.now() - t0, chrono::Duration::hours(3));
    }

    #[test]
    fn rfc3339_is_second_precision() {
        // Go 의 RFC3339 는 나노초가 0이면 소수부를 붙이지 않습니다. 해시 입력이
        // 여기에 의존하므로 형식을 고정합니다.
        let t = Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap();
        assert_eq!(to_rfc3339(t), "2026-07-10T09:00:00Z");
    }

    #[test]
    fn truncate_drops_subsecond() {
        let t = Utc
            .with_ymd_and_hms(2026, 7, 10, 9, 0, 0)
            .unwrap()
            .checked_add_signed(chrono::Duration::milliseconds(750))
            .unwrap();
        assert_eq!(to_rfc3339(truncate_to_second(t)), "2026-07-10T09:00:00Z");
    }
}
