//! 시스템별 서킷 브레이커.
//!
//! 연속 실패가 임계치를 넘으면 회로를 열어 레거시를 아예 부르지 않고 즉시
//! 실패시킵니다 (fail fast). 죽어가는 시스템에 부하를 더 얹지 않기 위함입니다.
//! 냉각 시간이 지나면 한 건만 통과시켜(half-open) 회복을 확인합니다.
//!
//! **업무 오류는 회로에 반영하지 않습니다** — 없는 송장을 여러 번 조회했다고
//! ERP가 죽은 것은 아니기 때문입니다. 그 판단은
//! 호출자([`crate::transient::is_temporary`])가
//! 하고, 이 모듈은 [`Breaker::failure`] 로 넘어온 것만 셉니다.
//!
//! ```text
//! closed    --(연속 실패 >= threshold)-->  open
//! open      --(cooldown 경과)-->           half-open
//! half-open --(성공)-->                    closed
//! half-open --(실패)-->                    open
//! ```

use crate::clock::{SharedClock,
                   SystemClock};
use chrono::{DateTime,
             Utc};
use std::{collections::HashMap,
          sync::{Arc,
                 Mutex},
          time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Closed,
    Open,
    HalfOpen,
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            | State::Closed => "closed",
            | State::Open => "open",
            | State::HalfOpen => "half-open",
        })
    }
}

/// 회로가 열려 있어 호출을 차단했음.
#[derive(Debug, Clone)]
pub struct ErrOpen {
    pub key: String,
    pub retry_in: Duration,
    pub failures: i64,
}

impl std::fmt::Display for ErrOpen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "circuit open for {:?} after {} failures; retry in {:?}",
            self.key,
            self.failures,
            Duration::from_secs(self.retry_in.as_secs())
        )
    }
}

impl std::error::Error for ErrOpen {}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub threshold: i64,
    pub cooldown: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            threshold: 5,
            cooldown: Duration::from_secs(30),
        }
    }
}

impl Config {
    fn normalized(self) -> Self {
        Self {
            threshold: if self.threshold <= 0 { 5 } else { self.threshold },
            cooldown: if self.cooldown.is_zero() { Duration::from_secs(30) } else { self.cooldown },
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Circuit {
    state: State,
    failures: i64,
    opened_at: Option<DateTime<Utc>>,
    last_failure: String,
}

/// 브레이커 상태 (운영 콘솔 표시용).
#[derive(Debug, Clone)]
pub struct Status {
    pub key: String,
    pub state: State,
    pub failures: i64,
    pub last_failure: String,
    pub retry_in: Duration,
}

/// 시스템별 서킷 브레이커.
pub struct Breaker {
    circuits: Mutex<HashMap<String, Circuit>>,
    cfg: Config,
    clock: SharedClock,
}

impl std::fmt::Debug for Breaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("Breaker").field("cfg", &self.cfg).finish() }
}

impl Default for Breaker {
    fn default() -> Self { Self::new(Config::default()) }
}

impl Breaker {
    pub fn new(cfg: Config) -> Self {
        Self {
            circuits: Mutex::new(HashMap::new()),
            cfg: cfg.normalized(),
            clock: Arc::new(SystemClock),
        }
    }

    pub fn with_clock(cfg: Config, clock: SharedClock) -> Self {
        Self {
            circuits: Mutex::new(HashMap::new()),
            cfg: cfg.normalized(),
            clock,
        }
    }

    /// 호출해도 되는지 묻습니다. 회로가 열려 있으면 [`ErrOpen`].
    ///
    /// 냉각 시간이 지났으면 **이 호출이 open → half-open 전이를 수행하고**
    /// 통과시킵니다.
    pub fn allow(&self, key: &str) -> Result<(), ErrOpen> {
        let mut circuits = self.circuits.lock().unwrap();
        let Some(c) = circuits.get_mut(key) else {
            return Ok(()); // 한 번도 실패한 적이 없습니다.
        };
        if c.state != State::Open {
            return Ok(());
        }
        let opened_at = c.opened_at.unwrap_or_else(|| self.clock.now());
        let elapsed = (self.clock.now() - opened_at).to_std().unwrap_or(Duration::ZERO);
        if elapsed < self.cfg.cooldown {
            return Err(ErrOpen {
                key: key.to_string(),
                retry_in: self.cfg.cooldown - elapsed,
                failures: c.failures,
            });
        }
        // 냉각 완료 — 한 건만 통과시켜 회복을 확인합니다.
        c.state = State::HalfOpen;
        Ok(())
    }

    /// 성공을 기록합니다. 회로를 닫고 실패 횟수를 초기화합니다.
    pub fn success(&self, key: &str) {
        let mut circuits = self.circuits.lock().unwrap();
        if let Some(c) = circuits.get_mut(key) {
            c.state = State::Closed;
            c.failures = 0;
            c.last_failure.clear();
        }
    }

    /// **일시적 장애만** 기록합니다. 업무 오류를 넘기면 안 됩니다.
    pub fn failure(&self, key: &str, err: &str) {
        let mut circuits = self.circuits.lock().unwrap();
        let c = circuits.entry(key.to_string()).or_default();
        c.failures += 1;
        c.last_failure = err.to_string();
        // half-open 에서의 실패는 횟수와 무관하게 즉시 회로를 다시 엽니다.
        if c.state == State::HalfOpen || c.failures >= self.cfg.threshold {
            c.state = State::Open;
            c.opened_at = Some(self.clock.now());
        }
    }

    /// 모든 회로 상태 (키순 정렬).
    pub fn statuses(&self) -> Vec<Status> {
        let circuits = self.circuits.lock().unwrap();
        let now = self.clock.now();
        let mut out: Vec<Status> = circuits
            .iter()
            .map(|(key, c)| {
                let retry_in = if c.state == State::Open {
                    let opened_at = c.opened_at.unwrap_or(now);
                    let elapsed = (now - opened_at).to_std().unwrap_or(Duration::ZERO);
                    self.cfg.cooldown.saturating_sub(elapsed)
                } else {
                    Duration::ZERO
                };
                Status {
                    key: key.clone(),
                    state: c.state,
                    failures: c.failures,
                    last_failure: c.last_failure.clone(),
                    retry_in,
                }
            })
            .collect();
        out.sort_by(|a, b| a.key.cmp(&b.key));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::TestClock;

    fn breaker(clock: Arc<TestClock>) -> Breaker {
        Breaker::with_clock(
            Config {
                threshold: 3,
                cooldown: Duration::from_secs(30),
            },
            clock,
        )
    }

    #[test]
    fn allows_when_never_failed() {
        let b = Breaker::default();
        assert!(b.allow("erp").is_ok());
    }

    #[test]
    fn opens_after_threshold_consecutive_failures() {
        let clock = Arc::new(TestClock::epoch());
        let b = breaker(clock);
        for _ in 0 .. 2 {
            b.failure("erp", "503");
        }
        assert!(b.allow("erp").is_ok()); // 아직 임계치 미만

        b.failure("erp", "503");
        let err = b.allow("erp").unwrap_err();
        assert_eq!(err.failures, 3);
    }

    #[test]
    fn success_resets_the_circuit() {
        let clock = Arc::new(TestClock::epoch());
        let b = breaker(clock);
        b.failure("erp", "503");
        b.failure("erp", "503");
        b.success("erp");
        b.failure("erp", "503");
        // 성공이 카운터를 지웠으므로 아직 열리지 않습니다.
        assert!(b.allow("erp").is_ok());
    }

    #[test]
    fn open_transitions_to_half_open_after_cooldown() {
        let clock = Arc::new(TestClock::epoch());
        let b = breaker(clock.clone());
        for _ in 0 .. 3 {
            b.failure("erp", "503");
        }
        assert!(b.allow("erp").is_err());

        clock.advance(chrono::Duration::seconds(31));
        // 냉각이 끝나면 한 건 통과시킵니다.
        assert!(b.allow("erp").is_ok());
        assert_eq!(b.statuses()[0].state, State::HalfOpen);
    }

    #[test]
    fn half_open_success_closes_the_circuit() {
        let clock = Arc::new(TestClock::epoch());
        let b = breaker(clock.clone());
        for _ in 0 .. 3 {
            b.failure("erp", "503");
        }
        clock.advance(chrono::Duration::seconds(31));
        b.allow("erp").unwrap();
        b.success("erp");
        assert_eq!(b.statuses()[0].state, State::Closed);
    }

    #[test]
    fn half_open_failure_reopens_immediately_regardless_of_count() {
        let clock = Arc::new(TestClock::epoch());
        let b = breaker(clock.clone());
        for _ in 0 .. 3 {
            b.failure("erp", "503");
        }
        clock.advance(chrono::Duration::seconds(31));
        b.allow("erp").unwrap(); // half-open
        b.failure("erp", "still down");
        assert!(b.allow("erp").is_err());
        assert_eq!(b.statuses()[0].state, State::Open);
    }

    #[test]
    fn retry_in_shrinks_as_cooldown_elapses() {
        let clock = Arc::new(TestClock::epoch());
        let b = breaker(clock.clone());
        for _ in 0 .. 3 {
            b.failure("erp", "503");
        }
        let first = b.allow("erp").unwrap_err().retry_in;
        clock.advance(chrono::Duration::seconds(10));
        let second = b.allow("erp").unwrap_err().retry_in;
        assert!(second < first);
    }

    #[test]
    fn circuits_are_per_system() {
        let clock = Arc::new(TestClock::epoch());
        let b = breaker(clock);
        for _ in 0 .. 3 {
            b.failure("erp", "503");
        }
        assert!(b.allow("erp").is_err());
        // ERP 가 죽었다고 CRM 을 막지 않습니다.
        assert!(b.allow("crm").is_ok());
    }

    #[test]
    fn statuses_are_sorted_and_report_last_failure() {
        let clock = Arc::new(TestClock::epoch());
        let b = breaker(clock);
        b.failure("erp", "boom");
        b.failure("crm", "bang");
        let st = b.statuses();
        assert_eq!(st[0].key, "crm");
        assert_eq!(st[1].key, "erp");
        assert_eq!(st[1].last_failure, "boom");
    }
}
