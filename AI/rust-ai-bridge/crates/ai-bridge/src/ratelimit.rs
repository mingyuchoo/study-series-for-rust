//! 분당 호출 제한 (메모리 · Redis).
//!
//! **Redis 에 닿지 못하면 fail-open 입니다** — 리소스 가드가 죽었다고 모든
//! 트래픽을 막으면 서비스가 함께 멈추므로, 이번 호출을 허용하고 로그만
//! 남깁니다.
//!
//! 권한·승인 같은 **인가 가드는 그대로 fail-closed** 입니다. 이 구분이
//! 중요합니다: 레이트 리밋은 "폭주를 막는 장치"이지 "권한을 판단하는 장치"가
//! 아닙니다.

use crate::clock::{SharedClock,
                   SystemClock};
use chrono::{DateTime,
             Utc};
use std::{collections::HashMap,
          sync::Mutex,
          time::Duration};

/// 분당 호출 제한.
#[async_trait::async_trait]
pub trait Limiter: Send + Sync {
    /// 한도 안이면 `true` 를 돌려주고 카운터를 올립니다.
    ///
    /// `limit <= 0` 이면 제한 없음.
    async fn allow(&self, key: &str, limit: i64) -> bool;
}

struct Window {
    start: DateTime<Utc>,
    count: i64,
}

/// 프로세스 메모리 카운터 (단일 인스턴스).
///
/// **고정 창(fixed window)** 입니다 — 창 경계를 걸치면 짧은 시간에 최대 2배가
/// 통과할 수 있습니다. 폭주 차단이 목적이므로 받아들이는 절충입니다.
pub struct Memory {
    windows: Mutex<HashMap<String, Window>>,
    clock: SharedClock,
}

impl std::fmt::Debug for Memory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("ratelimit::Memory").finish() }
}

impl Default for Memory {
    fn default() -> Self { Self::new() }
}

impl Memory {
    pub fn new() -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
            clock: std::sync::Arc::new(SystemClock),
        }
    }

    pub fn with_clock(clock: SharedClock) -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
            clock,
        }
    }
}

#[async_trait::async_trait]
impl Limiter for Memory {
    async fn allow(&self, key: &str, limit: i64) -> bool {
        if limit <= 0 {
            return true;
        }
        let now = self.clock.now();
        let mut windows = self.windows.lock().unwrap();
        let w = windows.entry(key.to_string()).or_insert(Window {
            start: now,
            count: 0,
        });
        if now - w.start >= chrono::Duration::minutes(1) {
            w.start = now;
            w.count = 0;
        }
        w.count += 1;
        w.count <= limit
    }
}

/// Redis 공유 카운터 (여러 인스턴스).
///
/// 공유하지 않으면 인스턴스가 늘어날수록 실효 한도가 배로 느슨해집니다.
pub struct RedisLimiter {
    client: redis::Client,
    prefix: String,
}

impl std::fmt::Debug for RedisLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("RedisLimiter").field("prefix", &self.prefix).finish() }
}

/// `INCR` 후 **첫 증가일 때만** 만료를 겁니다.
///
/// 읽고-증가시키는 두 번의 왕복으로 나누면 동시 호출이 한도를 넘습니다. Lua
/// 안에서 원자적으로 실행합니다.
const INCR_SCRIPT: &str = r#"
local c = redis.call('INCR', KEYS[1])
if c == 1 then
  redis.call('EXPIRE', KEYS[1], ARGV[1])
end
return c
"#;

impl RedisLimiter {
    pub async fn new(url: &str, prefix: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)?;
        // 기동 시 확인합니다 — 레이트 리밋 없이 조용히 도는 것보다 기동을 거부하는 편이
        // 낫습니다.
        let mut conn = tokio::time::timeout(Duration::from_secs(5), client.get_multiplexed_async_connection())
            .await
            .map_err(|_| anyhow::anyhow!("ratelimit: redis ping timed out"))??;
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("ratelimit: redis ping failed: {e}"))?;

        Ok(Self {
            client,
            prefix: if prefix.is_empty() { "ratelimit:".to_string() } else { prefix.to_string() },
        })
    }

    async fn incr(&self, key: &str) -> anyhow::Result<i64> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let n: i64 = redis::Script::new(INCR_SCRIPT)
            .key(format!("{}{}", self.prefix, key))
            .arg(60)
            .invoke_async(&mut conn)
            .await?;
        Ok(n)
    }
}

#[async_trait::async_trait]
impl Limiter for RedisLimiter {
    async fn allow(&self, key: &str, limit: i64) -> bool {
        if limit <= 0 {
            return true;
        }
        match tokio::time::timeout(Duration::from_secs(3), self.incr(key)).await {
            | Ok(Ok(n)) => n <= limit,
            | Ok(Err(e)) => {
                // **fail-open** — 가드가 죽었다고 서비스를 멈추지 않습니다.
                tracing::warn!("ratelimit: redis unavailable, allowing call: {e}");
                true
            },
            | Err(_) => {
                tracing::warn!("ratelimit: redis timed out, allowing call");
                true
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::TestClock;
    use std::sync::Arc;

    #[tokio::test]
    async fn allows_up_to_the_limit_then_denies() {
        let l = Memory::new();
        for _ in 0 .. 3 {
            assert!(l.allow("k", 3).await);
        }
        assert!(!l.allow("k", 3).await);
    }

    #[tokio::test]
    async fn keys_are_independent() {
        let l = Memory::new();
        assert!(l.allow("a", 1).await);
        assert!(l.allow("b", 1).await);
        assert!(!l.allow("a", 1).await);
    }

    #[tokio::test]
    async fn nonpositive_limit_is_unlimited() {
        let l = Memory::new();
        for _ in 0 .. 100 {
            assert!(l.allow("k", 0).await);
        }
    }

    #[tokio::test]
    async fn window_resets_after_a_minute() {
        let clock = Arc::new(TestClock::epoch());
        let l = Memory::with_clock(clock.clone());
        assert!(l.allow("k", 1).await);
        assert!(!l.allow("k", 1).await);

        clock.advance(chrono::Duration::seconds(61));
        assert!(l.allow("k", 1).await, "창이 지나면 다시 허용되어야 합니다");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn concurrent_calls_never_exceed_the_limit() {
        let l = Arc::new(Memory::new());
        let barrier = Arc::new(tokio::sync::Barrier::new(50));
        let mut hs = Vec::new();
        for _ in 0 .. 50 {
            let (l, b) = (l.clone(), barrier.clone());
            hs.push(tokio::spawn(async move {
                b.wait().await;
                l.allow("k", 10).await
            }));
        }
        let mut allowed = 0;
        for h in hs {
            if h.await.unwrap() {
                allowed += 1;
            }
        }
        assert_eq!(allowed, 10, "동시 50회 중 정확히 10회만 통과해야 합니다");
    }
}
