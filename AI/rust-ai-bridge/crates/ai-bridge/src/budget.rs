//! 세션별 LLM 비용 상한 (메모리 · Redis).
//!
//! 상한을 넘긴 세션의 **다음** 도구 호출부터 거부합니다. 이미 쓴 비용은 되돌릴
//! 수 없으므로, 상한은 **"여기서 멈춘다"는 뜻이지 "여기를 넘지 않는다"는 뜻이
//! 아닙니다.**
//!
//! 거부된 호출의 LLM 비용도 이미 발생했으므로 함께 누적합니다.
//!
//! Redis 에 닿지 못하면 [`ratelimit`](crate::ratelimit) 과 같이 **fail-open**
//! 입니다.

use std::{collections::HashMap,
          sync::Mutex,
          time::Duration};

/// 상한 초과.
#[derive(Debug, Clone, thiserror::Error)]
#[error("budget: 세션 {key:?} 이(가) 비용 상한을 초과했습니다(사용 {spent_micros}, 상한 {limit_micros} micros)")]
pub struct Exceeded {
    pub key: String,
    pub spent_micros: i64,
    pub limit_micros: i64,
}

/// 세션별 누적 비용.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub key: String,
    pub spent_micros: i64,
}

/// 비용 추적기.
#[async_trait::async_trait]
pub trait Tracker: Send + Sync {
    /// 상한을 넘었으면 [`Exceeded`].
    async fn check(&self, key: &str) -> Result<(), Exceeded>;
    /// 비용을 누적하고 새 누계를 돌려줍니다.
    async fn add(&self, key: &str, micros: i64) -> i64;
    /// 남은 예산. 상한이 없으면 `-1`.
    async fn remaining(&self, key: &str) -> i64;
    /// 상한. `0` 이면 무제한.
    fn limit(&self) -> i64;
    /// 비용 내림차순.
    async fn snapshot(&self) -> Vec<Entry>;
}

/// 프로세스 메모리 (단일 인스턴스).
#[derive(Debug)]
pub struct Memory {
    spent: Mutex<HashMap<String, i64>>,
    limit: i64,
}

impl Memory {
    /// `limit_micros <= 0` 이면 무제한.
    pub fn new(limit_micros: i64) -> Self {
        Self {
            spent: Mutex::new(HashMap::new()),
            limit: limit_micros.max(0),
        }
    }
}

#[async_trait::async_trait]
impl Tracker for Memory {
    async fn check(&self, key: &str) -> Result<(), Exceeded> {
        if self.limit <= 0 {
            return Ok(());
        }
        let spent = *self.spent.lock().unwrap().get(key).unwrap_or(&0);
        // 상한과 정확히 같아도 막습니다 — 이미 다 썼기 때문입니다.
        if spent >= self.limit {
            return Err(Exceeded {
                key: key.to_string(),
                spent_micros: spent,
                limit_micros: self.limit,
            });
        }
        Ok(())
    }

    async fn add(&self, key: &str, micros: i64) -> i64 {
        if micros == 0 {
            // 비용을 보고하지 않은 세션으로 스냅샷을 더럽히지 않습니다.
            return 0;
        }
        let mut spent = self.spent.lock().unwrap();
        let e = spent.entry(key.to_string()).or_insert(0);
        *e += micros;
        *e
    }

    async fn remaining(&self, key: &str) -> i64 {
        if self.limit <= 0 {
            return -1;
        }
        let spent = *self.spent.lock().unwrap().get(key).unwrap_or(&0);
        (self.limit - spent).max(0)
    }

    fn limit(&self) -> i64 { self.limit }

    async fn snapshot(&self) -> Vec<Entry> {
        let spent = self.spent.lock().unwrap();
        let mut out: Vec<Entry> = spent
            .iter()
            .map(|(k, v)| Entry {
                key: k.clone(),
                spent_micros: *v,
            })
            .collect();
        out.sort_by_key(|e| std::cmp::Reverse(e.spent_micros));
        out
    }
}

/// Redis 공유 카운터 (여러 인스턴스).
///
/// 상한 자체는 Redis 가 아니라 프로세스 설정에 있으므로, **모든 인스턴스가 같은
/// `-session-budget-micros` 로 떠야** 합의된 상한이 됩니다.
pub struct RedisTracker {
    client: redis::Client,
    hash_key: String,
    limit: i64,
}

impl std::fmt::Debug for RedisTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisTracker")
            .field("hash_key", &self.hash_key)
            .field("limit", &self.limit)
            .finish()
    }
}

impl RedisTracker {
    pub async fn new(url: &str, hash_key: &str, limit_micros: i64) -> anyhow::Result<Self> {
        let client = redis::Client::open(url)?;
        let mut conn = tokio::time::timeout(Duration::from_secs(5), client.get_multiplexed_async_connection())
            .await
            .map_err(|_| anyhow::anyhow!("budget: redis ping timed out"))??;
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("budget: redis ping failed: {e}"))?;

        Ok(Self {
            client,
            hash_key: if hash_key.is_empty() { "budget".to_string() } else { hash_key.to_string() },
            limit: limit_micros.max(0),
        })
    }

    async fn spent(&self, key: &str) -> anyhow::Result<i64> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let v: Option<i64> = redis::cmd("HGET").arg(&self.hash_key).arg(key).query_async(&mut conn).await?;
        Ok(v.unwrap_or(0))
    }
}

#[async_trait::async_trait]
impl Tracker for RedisTracker {
    async fn check(&self, key: &str) -> Result<(), Exceeded> {
        if self.limit <= 0 {
            return Ok(());
        }
        match tokio::time::timeout(Duration::from_secs(3), self.spent(key)).await {
            | Ok(Ok(spent)) if spent >= self.limit => Err(Exceeded {
                key: key.to_string(),
                spent_micros: spent,
                limit_micros: self.limit,
            }),
            | Ok(Ok(_)) => Ok(()),
            // fail-open.
            | Ok(Err(e)) => {
                tracing::warn!("budget: redis unavailable, allowing call: {e}");
                Ok(())
            },
            | Err(_) => {
                tracing::warn!("budget: redis timed out, allowing call");
                Ok(())
            },
        }
    }

    async fn add(&self, key: &str, micros: i64) -> i64 {
        if micros == 0 {
            return 0;
        }
        let fut = async {
            let mut conn = self.client.get_multiplexed_async_connection().await?;
            // HINCRBY 는 Redis 안에서 원자적입니다.
            let n: i64 = redis::cmd("HINCRBY").arg(&self.hash_key).arg(key).arg(micros).query_async(&mut conn).await?;
            Ok::<i64, redis::RedisError>(n)
        };
        match tokio::time::timeout(Duration::from_secs(3), fut).await {
            | Ok(Ok(n)) => n,
            | _ => {
                tracing::warn!("budget: redis unavailable, cost not accumulated");
                0
            },
        }
    }

    async fn remaining(&self, key: &str) -> i64 {
        if self.limit <= 0 {
            return -1;
        }
        match tokio::time::timeout(Duration::from_secs(3), self.spent(key)).await {
            | Ok(Ok(spent)) => (self.limit - spent).max(0),
            | _ => self.limit,
        }
    }

    fn limit(&self) -> i64 { self.limit }

    async fn snapshot(&self) -> Vec<Entry> {
        let fut = async {
            let mut conn = self.client.get_multiplexed_async_connection().await?;
            let m: HashMap<String, i64> = redis::cmd("HGETALL").arg(&self.hash_key).query_async(&mut conn).await?;
            Ok::<_, redis::RedisError>(m)
        };
        let Ok(Ok(m)) = tokio::time::timeout(Duration::from_secs(3), fut).await else {
            return Vec::new();
        };
        let mut out: Vec<Entry> = m
            .into_iter()
            .map(|(key, spent_micros)| Entry {
                key,
                spent_micros,
            })
            .collect();
        out.sort_by_key(|e| std::cmp::Reverse(e.spent_micros));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn no_limit_never_blocks() {
        let t = Memory::new(0);
        t.add("s", 1_000_000).await;
        assert!(t.check("s").await.is_ok());
        assert_eq!(t.remaining("s").await, -1);
    }

    #[tokio::test]
    async fn blocks_only_after_exceeding() {
        let t = Memory::new(1000);
        t.add("s", 400).await;
        assert!(t.check("s").await.is_ok());
        assert_eq!(t.remaining("s").await, 600);

        t.add("s", 700).await; // 누계 1100
        let err = t.check("s").await.unwrap_err();
        assert_eq!(err.spent_micros, 1100);
        assert_eq!(err.limit_micros, 1000);
        // 남은 예산은 음수가 되지 않습니다.
        assert_eq!(t.remaining("s").await, 0);
    }

    #[tokio::test]
    async fn keys_are_independent() {
        let t = Memory::new(100);
        t.add("hungry", 500).await;
        assert!(t.check("hungry").await.is_err());
        assert!(t.check("thrifty").await.is_ok());
    }

    #[tokio::test]
    async fn zero_cost_creates_no_entry() {
        // 비용을 보고하지 않는 오케스트레이터가 스냅샷을 더럽히지 않습니다.
        let t = Memory::new(1000);
        t.add("s", 0).await;
        assert!(t.snapshot().await.is_empty());
    }

    #[tokio::test]
    async fn snapshot_is_sorted_by_spend_descending() {
        let t = Memory::new(0);
        t.add("cheap", 10).await;
        t.add("expensive", 1000).await;
        t.add("medium", 100).await;
        let keys: Vec<String> = t.snapshot().await.into_iter().map(|e| e.key).collect();
        assert_eq!(keys, vec!["expensive", "medium", "cheap"]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn concurrent_adds_never_lose_updates() {
        let t = Arc::new(Memory::new(0));
        let barrier = Arc::new(tokio::sync::Barrier::new(100));
        let mut hs = Vec::new();
        for _ in 0 .. 100 {
            let (t, b) = (t.clone(), barrier.clone());
            hs.push(tokio::spawn(async move {
                b.wait().await;
                t.add("s", 10).await;
            }));
        }
        for h in hs {
            h.await.unwrap();
        }
        let snap = t.snapshot().await;
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].spent_micros, 1000, "동시 누적에서 갱신이 유실됐습니다");
    }
}
