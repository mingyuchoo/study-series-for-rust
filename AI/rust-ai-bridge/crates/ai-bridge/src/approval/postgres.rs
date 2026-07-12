//! PostgreSQL 승인 저장소 (분산 배포).
//!
//! SQLite 는 `BEGIN IMMEDIATE` 로 DB 전체를 직렬화하지만, PostgreSQL 은
//! **`SELECT … FOR UPDATE`** 로 해당 지문의 행만 잠급니다 — 서로 다른 지문의
//! 동시 `ensure` 는 서로를 막지 않습니다. **결과(승인은 한 번만 소비된다)는
//! 같습니다.**
//!
//! `rowid` 가 없으므로 "가장 최근" 정렬을 위해 `seq BIGSERIAL` 을 둡니다.

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
           Postgres,
           Row,
           Transaction,
           postgres::{PgPool,
                      PgPoolOptions}};
use std::time::Duration;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS approval_request (
    seq          BIGSERIAL,
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
    ttl_seconds  BIGINT NOT NULL DEFAULT 0,
    expires_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_approval_fingerprint ON approval_request(fingerprint);
CREATE INDEX IF NOT EXISTS idx_approval_status ON approval_request(status);
"#;

const COLS: &str = "id, fingerprint, actor, tool, args_json, status, requested_at,
                    decided_by, decided_at, note, ttl_seconds, expires_at";

pub struct PostgresApprovalStore {
    pool: PgPool,
    clock: SharedClock,
}

impl std::fmt::Debug for PostgresApprovalStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.debug_struct("PostgresApprovalStore").finish() }
}

impl PostgresApprovalStore {
    pub async fn open(dsn: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new().max_connections(10).connect(dsn).await?;
        Self::from_pool(pool).await
    }

    pub async fn from_pool(pool: PgPool) -> anyhow::Result<Self> {
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

    pub fn set_clock(&mut self, clock: SharedClock) { self.clock = clock; }

    pub fn pool(&self) -> &PgPool { &self.pool }
}

#[async_trait::async_trait]
impl Store for PostgresApprovalStore {
    async fn ensure(&self, actor: &str, tool: &str, args: &Map<String, Value>, ttl: Duration) -> Result<Request, Error> {
        let ttl = effective_ttl(ttl);
        let fp = fingerprint(actor, tool, args);
        let args_json = serde_json::to_string(&Value::Object(args.clone())).map_err(|e| Error::Other(e.into()))?;
        let now = self.clock.now();

        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await.map_err(sql)?;

        // FOR UPDATE — 같은 지문의 행만 잠급니다.
        let existing = sqlx::query(AssertSqlSafe(format!(
            "SELECT {COLS} FROM approval_request
             WHERE fingerprint = $1 AND status IN ('pending','approved','rejected')
             ORDER BY seq DESC LIMIT 1
             FOR UPDATE"
        )))
        .bind(&fp)
        .fetch_optional(&mut *tx)
        .await
        .map_err(sql)?;

        let out = match existing {
            | None => insert_pending(&mut tx, &fp, actor, tool, &args_json, ttl, now).await?,
            | Some(row) => {
                let req = request_from_row(&row)?;
                match req.status {
                    | Status::Pending | Status::Rejected => req,

                    | Status::Approved if req.expired(now) => {
                        sqlx::query(
                            "UPDATE approval_request SET status='expired'
                             WHERE id = $1 AND status='approved'",
                        )
                        .bind(&req.id)
                        .execute(&mut *tx)
                        .await
                        .map_err(sql)?;
                        insert_pending(&mut tx, &fp, actor, tool, &args_json, ttl, now).await?
                    },

                    | Status::Approved => {
                        let res = sqlx::query(
                            "UPDATE approval_request SET status='consumed', consumed_at=$1
                             WHERE id = $2 AND status='approved'",
                        )
                        .bind(clock::to_rfc3339(now))
                        .bind(&req.id)
                        .execute(&mut *tx)
                        .await
                        .map_err(sql)?;
                        if res.rows_affected() != 1 {
                            return Err(Error::Raced);
                        }
                        Request {
                            status: Status::Approved,
                            ..req
                        }
                    },

                    | Status::Consumed | Status::Expired => insert_pending(&mut tx, &fp, actor, tool, &args_json, ttl, now).await?,
                }
            },
        };

        tx.commit().await.map_err(sql)?;
        Ok(out)
    }

    async fn decide(&self, id: &str, approve: bool, by: &str, note: &str) -> Result<Request, Error> {
        if by.is_empty() {
            return Err(Error::NoApprover);
        }
        let now = self.clock.now();
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await.map_err(sql)?;

        let row = sqlx::query("SELECT actor, status, ttl_seconds FROM approval_request WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(sql)?
            .ok_or(Error::NotFound)?;

        let actor: String = row.try_get("actor").map_err(sql)?;
        let status: String = row.try_get("status").map_err(sql)?;
        let ttl_seconds: i64 = row.try_get("ttl_seconds").map_err(sql)?;

        if status != "pending" {
            return Err(Error::NotPending);
        }
        // 검사와 상태 전이가 같은 트랜잭션 안에 있어야 합니다.
        if by == actor {
            return Err(Error::SelfApproval(actor));
        }

        let new_status = if approve { "approved" } else { "rejected" };
        let expires_at = if approve {
            let ttl = if ttl_seconds <= 0 {
                super::DEFAULT_TTL
            } else {
                Duration::from_secs(ttl_seconds as u64)
            };
            // 시계는 결정 시점부터 흐릅니다.
            Some(clock::to_rfc3339(now + chrono::Duration::from_std(ttl).unwrap_or_default()))
        } else {
            None
        };

        sqlx::query(
            "UPDATE approval_request
             SET status=$1, decided_by=$2, decided_at=$3, note=$4, expires_at=$5
             WHERE id = $6 AND status='pending'",
        )
        .bind(new_status)
        .bind(by)
        .bind(clock::to_rfc3339(now))
        .bind(note)
        .bind(expires_at)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(sql)?;

        tx.commit().await.map_err(sql)?;
        self.get(id).await
    }

    async fn get(&self, id: &str) -> Result<Request, Error> {
        let row = sqlx::query(AssertSqlSafe(format!("SELECT {COLS} FROM approval_request WHERE id = $1")))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(sql)?
            .ok_or(Error::NotFound)?;
        request_from_row(&row)
    }

    async fn list(&self, status: Option<Status>, limit: i64) -> Result<Vec<Request>, Error> {
        let limit = if limit <= 0 { 50 } else { limit };
        let rows = match status {
            | Some(s) =>
                sqlx::query(AssertSqlSafe(format!(
                    "SELECT {COLS} FROM approval_request WHERE status = $1
                     ORDER BY seq DESC LIMIT $2"
                )))
                .bind(s.as_str())
                .bind(limit)
                .fetch_all(&self.pool)
                .await,
            | None =>
                sqlx::query(AssertSqlSafe(format!("SELECT {COLS} FROM approval_request ORDER BY seq DESC LIMIT $1")))
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await,
        }
        .map_err(sql)?;
        rows.iter().map(request_from_row).collect()
    }
}

async fn insert_pending(
    tx: &mut Transaction<'_, Postgres>,
    fp: &str,
    actor: &str,
    tool: &str,
    args_json: &str,
    ttl: Duration,
    now: DateTime<Utc>,
) -> Result<Request, Error> {
    let id = new_id();
    sqlx::query(
        "INSERT INTO approval_request
            (id, fingerprint, actor, tool, args_json, status, requested_at, ttl_seconds)
         VALUES ($1,$2,$3,$4,$5,'pending',$6,$7)",
    )
    .bind(&id)
    .bind(fp)
    .bind(actor)
    .bind(tool)
    .bind(args_json)
    .bind(clock::to_rfc3339(now))
    .bind(ttl.as_secs() as i64)
    .execute(&mut **tx)
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

fn request_from_row(r: &sqlx::postgres::PgRow) -> Result<Request, Error> {
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
