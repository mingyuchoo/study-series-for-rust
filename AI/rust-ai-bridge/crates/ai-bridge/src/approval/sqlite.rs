//! SQLite 승인 저장소 (단일 노드).
//!
//! `ensure` 의 "읽고-소비한다" 구간은 **`BEGIN IMMEDIATE`** 로 직렬화합니다.
//! 평범한 `BEGIN`(deferred)은 첫 읽기에서 쓰기 잠금을 잡지 않으므로, 두
//! 프로세스가 같은 승인을 읽고 둘 다 소비할 수 있습니다 — 그러면 도구가 두 번
//! 실행됩니다.

use super::{Error,
            Request,
            Status,
            Store,
            effective_ttl,
            fingerprint,
            new_id};
use crate::clock::{self,
                   SharedClock,
                   SystemClock};
use anyhow::anyhow;
use chrono::{DateTime,
             Utc};
use serde_json::{Map,
                 Value};
use sqlx::{AssertSqlSafe,
           Row,
           sqlite::{SqliteConnectOptions,
                    SqliteJournalMode,
                    SqlitePool,
                    SqlitePoolOptions}};
use std::{path::Path,
          time::Duration};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS approval_request (
    id           TEXT PRIMARY KEY,
    fingerprint  TEXT NOT NULL,
    actor        TEXT NOT NULL,
    tool         TEXT NOT NULL,
    args_json    TEXT NOT NULL,
    status       TEXT NOT NULL,
    requested_at TEXT NOT NULL,
    decided_by   TEXT,
    decided_at   TEXT,
    note         TEXT,
    consumed_at  TEXT,
    ttl_seconds  INTEGER NOT NULL DEFAULT 0,
    expires_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_approval_fingerprint ON approval_request(fingerprint);
CREATE INDEX IF NOT EXISTS idx_approval_status ON approval_request(status);
"#;

const COLS: &str = "id, fingerprint, actor, tool, args_json, status, requested_at,
                    decided_by, decided_at, note, ttl_seconds, expires_at";

/// SQLite 승인 저장소.
pub struct SqliteApprovalStore {
    pool: SqlitePool,
    clock: SharedClock,
}

impl std::fmt::Debug for SqliteApprovalStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("SqliteApprovalStore").finish() }
}

impl SqliteApprovalStore {
    pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        // `BEGIN IMMEDIATE` 가 원자성을 보장하므로 연결을 여러 개 둬도 안전합니다.
        let pool = SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?;
        Self::from_pool(pool).await
    }

    /// 인메모리 DB. **연결마다 별도 DB 가 되므로 연결은 하나뿐입니다** — 동시성
    /// 검사에는 파일 기반 DB([`SqliteApprovalStore::open`])를 쓰십시오.
    pub async fn open_in_memory() -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::new().in_memory(true);
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
        Self::from_pool(pool).await
    }

    pub async fn from_pool(pool: SqlitePool) -> anyhow::Result<Self> {
        for stmt in SCHEMA.split(';') {
            let s = stmt.trim();
            if !s.is_empty() {
                sqlx::query(AssertSqlSafe(s.to_string())).execute(&pool).await?;
            }
        }
        Ok(Self {
            pool,
            clock: std::sync::Arc::new(SystemClock),
        })
    }

    /// 테스트용 시계를 꽂습니다 — 만료를 기다리지 않고 검사하기 위함입니다.
    pub fn set_clock(&mut self, clock: SharedClock) { self.clock = clock; }

    pub fn pool(&self) -> &SqlitePool { &self.pool }
}

#[async_trait::async_trait]
impl Store for SqliteApprovalStore {
    async fn ensure(&self, actor: &str, tool: &str, args: &Map<String, Value>, ttl: Duration) -> Result<Request, Error> {
        let ttl = effective_ttl(ttl);
        let fp = fingerprint(actor, tool, args);
        let args_json = serde_json::to_string(&Value::Object(args.clone())).map_err(|e| Error::Other(e.into()))?;
        let now = self.clock.now();

        let mut conn = self.pool.acquire().await.map_err(sql)?;

        // 읽기부터 쓰기 잠금을 잡습니다. 이것이 "승인은 단 한 번만 소비된다"의
        // 근거입니다.
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await.map_err(sql)?;

        let result: Result<Request, Error> = async {
            // 살아 있는 요청만 봅니다 — consumed/expired 는 종결 상태이므로 새로 만듭니다.
            let existing = sqlx::query(AssertSqlSafe(format!(
                "SELECT {COLS} FROM approval_request
                 WHERE fingerprint = ? AND status IN ('pending','approved','rejected')
                 ORDER BY rowid DESC LIMIT 1"
            )))
            .bind(&fp)
            .fetch_optional(&mut *conn)
            .await
            .map_err(sql)?;

            let Some(row) = existing else {
                return insert_pending(&mut conn, &fp, actor, tool, &args_json, ttl, now).await;
            };
            let req = request_from_row(&row)?;

            match req.status {
                // 대기·거부는 그대로 — 중복 요청을 만들지 않습니다.
                | Status::Pending | Status::Rejected => Ok(req),

                | Status::Approved if req.expired(now) => {
                    // 만료된 승인으로는 실행할 수 없습니다. 옛 요청을 만료 처리하고
                    // **새 대기 요청**을 만들어 관리자가 지금 상황에서 다시 판단하게 합니다.
                    sqlx::query(
                        "UPDATE approval_request SET status='expired'
                         WHERE id = ? AND status='approved'",
                    )
                    .bind(&req.id)
                    .execute(&mut *conn)
                    .await
                    .map_err(sql)?;

                    insert_pending(&mut conn, &fp, actor, tool, &args_json, ttl, now).await
                },

                | Status::Approved => {
                    // 소비합니다. 조건부 UPDATE 라 동시 소비 중 하나만 성공합니다.
                    let res = sqlx::query(
                        "UPDATE approval_request SET status='consumed', consumed_at=?
                         WHERE id = ? AND status='approved'",
                    )
                    .bind(clock::to_rfc3339(now))
                    .bind(&req.id)
                    .execute(&mut *conn)
                    .await
                    .map_err(sql)?;

                    if res.rows_affected() != 1 {
                        return Err(Error::Raced);
                    }
                    // 호출자에게는 "지금 실행해도 된다"는 뜻의 Approved 를 돌려주되,
                    // 저장소에는 consumed 로 남겨 두 번 쓰이지 못하게 합니다.
                    Ok(Request {
                        status: Status::Approved,
                        ..req
                    })
                },

                // 위 SELECT 가 걸러내므로 도달하지 않습니다.
                | Status::Consumed | Status::Expired => insert_pending(&mut conn, &fp, actor, tool, &args_json, ttl, now).await,
            }
        }
        .await;

        match result {
            | Ok(req) => {
                sqlx::query("COMMIT").execute(&mut *conn).await.map_err(sql)?;
                Ok(req)
            },
            | Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            },
        }
    }

    async fn decide(&self, id: &str, approve: bool, by: &str, note: &str) -> Result<Request, Error> {
        if by.is_empty() {
            return Err(Error::NoApprover);
        }
        let now = self.clock.now();
        let mut conn = self.pool.acquire().await.map_err(sql)?;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await.map_err(sql)?;

        let result: Result<(), Error> = async {
            let row = sqlx::query("SELECT actor, status, ttl_seconds FROM approval_request WHERE id = ?")
                .bind(id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(sql)?
                .ok_or(Error::NotFound)?;

            let actor: String = row.try_get("actor").map_err(sql)?;
            let status: String = row.try_get("status").map_err(sql)?;
            let ttl_seconds: i64 = row.try_get("ttl_seconds").map_err(sql)?;

            if status != "pending" {
                return Err(Error::NotPending);
            }
            // **승인 관문의 핵심.** 검사와 상태 전이가 같은 트랜잭션 안에 있어야
            // 검사와 갱신 사이에 다른 결정이 끼어들지 못합니다. 거부도 함께 막습니다 —
            // 요청자가 거부·재요청을 반복해 승인 이력을 흐릴 수 있기 때문입니다.
            if by == actor {
                return Err(Error::SelfApproval(actor));
            }

            let new_status = if approve { "approved" } else { "rejected" };
            // 승인된 것만 만료 시각을 가집니다. 거부된 요청은 만료될 것이 없습니다.
            let expires_at = if approve {
                let ttl = if ttl_seconds <= 0 {
                    super::DEFAULT_TTL
                } else {
                    Duration::from_secs(ttl_seconds as u64)
                };
                // 시계는 **결정 시점**부터 흐릅니다.
                Some(clock::to_rfc3339(now + chrono::Duration::from_std(ttl).unwrap_or_default()))
            } else {
                None
            };

            sqlx::query(
                "UPDATE approval_request
                 SET status=?, decided_by=?, decided_at=?, note=?, expires_at=?
                 WHERE id = ? AND status='pending'",
            )
            .bind(new_status)
            .bind(by)
            .bind(clock::to_rfc3339(now))
            .bind(note)
            .bind(expires_at)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(sql)?;
            Ok(())
        }
        .await;

        match result {
            | Ok(()) => {
                sqlx::query("COMMIT").execute(&mut *conn).await.map_err(sql)?;
            },
            | Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(e);
            },
        }
        drop(conn);
        self.get(id).await
    }

    async fn get(&self, id: &str) -> Result<Request, Error> {
        let row = sqlx::query(AssertSqlSafe(format!("SELECT {COLS} FROM approval_request WHERE id = ?")))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(sql)?
            .ok_or(Error::NotFound)?;
        request_from_row(&row)
    }

    async fn list(&self, status: Option<Status>, limit: i64) -> Result<Vec<Request>, Error> {
        let limit = if limit <= 0 { 50 } else { limit };
        let mut sql_text = format!("SELECT {COLS} FROM approval_request");
        if status.is_some() {
            sql_text.push_str(" WHERE status = ?");
        }
        sql_text.push_str(" ORDER BY rowid DESC LIMIT ?");

        let mut q = sqlx::query(AssertSqlSafe(sql_text));
        if let Some(s) = status {
            q = q.bind(s.as_str());
        }
        q = q.bind(limit);

        let rows = q.fetch_all(&self.pool).await.map_err(sql)?;
        rows.iter().map(request_from_row).collect()
    }
}

async fn insert_pending(
    conn: &mut sqlx::SqliteConnection,
    fp: &str,
    actor: &str,
    tool: &str,
    args_json: &str,
    ttl: Duration,
    now: DateTime<Utc>,
) -> Result<Request, Error> {
    let id = new_id();
    // TTL 은 **요청 시점**에 굳힙니다 — 배포로 기본 TTL 이 바뀌어도 관리자가 본
    // 값이 소급해서 달라지지 않게 하기 위함입니다.
    sqlx::query(
        "INSERT INTO approval_request
            (id, fingerprint, actor, tool, args_json, status, requested_at, ttl_seconds)
         VALUES (?,?,?,?,?, 'pending', ?, ?)",
    )
    .bind(&id)
    .bind(fp)
    .bind(actor)
    .bind(tool)
    .bind(args_json)
    .bind(clock::to_rfc3339(now))
    .bind(ttl.as_secs() as i64)
    .execute(&mut *conn)
    .await
    .map_err(sql)?;

    Ok(Request {
        id,
        fingerprint: fp.to_string(),
        actor: actor.to_string(),
        tool: tool.to_string(),
        args: serde_json::from_str::<Value>(args_json)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default(),
        status: Status::Pending,
        requested_at: clock::truncate_to_second(now),
        decided_by: String::new(),
        decided_at: None,
        note: String::new(),
        ttl,
        expires_at: None,
    })
}

fn sql(e: impl std::fmt::Display) -> Error { Error::Other(anyhow!("approval: {e}")) }

fn request_from_row(r: &sqlx::sqlite::SqliteRow) -> Result<Request, Error> {
    let status: String = r.try_get("status").map_err(sql)?;
    let requested_at: String = r.try_get("requested_at").map_err(sql)?;
    let args_json: String = r.try_get("args_json").map_err(sql)?;
    let ttl_seconds: i64 = r.try_get("ttl_seconds").map_err(sql)?;

    Ok(Request {
        id: r.try_get("id").map_err(sql)?,
        fingerprint: r.try_get("fingerprint").map_err(sql)?,
        actor: r.try_get("actor").map_err(sql)?,
        tool: r.try_get("tool").map_err(sql)?,
        args: serde_json::from_str::<Value>(&args_json)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default(),
        status: Status::parse(&status).ok_or_else(|| Error::Other(anyhow!("approval: unknown status {status:?}")))?,
        requested_at: clock::parse_rfc3339(&requested_at).ok_or_else(|| Error::Other(anyhow!("approval: bad requested_at")))?,
        decided_by: r.try_get::<Option<String>, _>("decided_by").map_err(sql)?.unwrap_or_default(),
        decided_at: r
            .try_get::<Option<String>, _>("decided_at")
            .map_err(sql)?
            .and_then(|s| clock::parse_rfc3339(&s)),
        note: r.try_get::<Option<String>, _>("note").map_err(sql)?.unwrap_or_default(),
        ttl: Duration::from_secs(ttl_seconds.max(0) as u64),
        expires_at: r
            .try_get::<Option<String>, _>("expires_at")
            .map_err(sql)?
            .and_then(|s| clock::parse_rfc3339(&s)),
    })
}
