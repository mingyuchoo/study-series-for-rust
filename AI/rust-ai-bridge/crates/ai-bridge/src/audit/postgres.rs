//! PostgreSQL 감사 로거 (분산 배포).
//!
//! SQLite 판과 같은 계약을 지킵니다 — `storetest` 적합성 스위트가 두 구현에
//! 똑같이 걸립니다.
//!
//! 다른 점은 동시성 제어입니다. SQLite 는 연결 하나로 체인 꼬리 갱신을
//! 직렬화하지만, PostgreSQL 은 여러 게이트웨이 인스턴스가 동시에 쓰므로
//! **advisory lock** 으로 `SELECT 꼬리 → INSERT` 구간을 직렬화합니다. 그러지
//! 않으면 두 인스턴스가 같은 `prev_hash` 를 읽어 체인이 갈라집니다.

use super::{Entry,
            Exporter,
            Filter,
            GroupBy,
            Policy,
            Purged,
            Purger,
            Reader,
            Recorder,
            Stat,
            Store,
            retention::{self,
                        PurgeBackend}};
use crate::clock;
use anyhow::{Result,
             anyhow,
             bail};
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
use std::collections::HashMap;

/// 체인 꼬리를 직렬화하는 advisory lock 키 (임의의 고정값).
const CHAIN_LOCK: i64 = 74201931;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS audit_log (
    id              BIGSERIAL PRIMARY KEY,
    ts              TEXT NOT NULL,
    actor           TEXT NOT NULL,
    tool            TEXT NOT NULL,
    system          TEXT NOT NULL,
    access          TEXT NOT NULL,
    decision        TEXT NOT NULL,
    reason          TEXT,
    approval_status TEXT,
    approval_id     TEXT,
    request_id      TEXT,
    session_id      TEXT,
    input_tokens    BIGINT NOT NULL DEFAULT 0,
    output_tokens   BIGINT NOT NULL DEFAULT 0,
    cost_micros     BIGINT NOT NULL DEFAULT 0,
    masked          INTEGER NOT NULL DEFAULT 0,
    input_json      TEXT,
    output_json     TEXT,
    latency_ms      BIGINT NOT NULL DEFAULT 0,
    error           TEXT,
    prompt          TEXT,
    injection       TEXT
);
CREATE INDEX IF NOT EXISTS idx_audit_actor ON audit_log(actor);
CREATE INDEX IF NOT EXISTS idx_audit_tool ON audit_log(tool);
CREATE INDEX IF NOT EXISTS idx_audit_ts ON audit_log(ts);
CREATE INDEX IF NOT EXISTS idx_audit_session ON audit_log(session_id);

CREATE TABLE IF NOT EXISTS audit_integrity (
    audit_id   BIGINT PRIMARY KEY REFERENCES audit_log(id) ON DELETE CASCADE,
    prev_hash  TEXT NOT NULL,
    entry_hash TEXT NOT NULL
);
"#;

/// PostgreSQL 감사 로거.
#[derive(Debug, Clone)]
pub struct PostgresLogger {
    pool: PgPool,
}

impl PostgresLogger {
    pub async fn open(dsn: &str) -> Result<Self> {
        let pool = PgPoolOptions::new().max_connections(10).connect(dsn).await?;
        Self::from_pool(pool).await
    }

    pub async fn from_pool(pool: PgPool) -> Result<Self> {
        for stmt in SCHEMA.split(';') {
            let s = stmt.trim();
            if !s.is_empty() {
                sqlx::query(s).execute(&pool).await?;
            }
        }
        Ok(Self {
            pool,
        })
    }

    pub fn pool(&self) -> &PgPool { &self.pool }

    pub async fn verify_integrity(&self) -> Result<()> {
        let rows = sqlx::query(
            "SELECT a.id, a.ts, a.actor, a.tool, a.system, a.access, a.decision, a.reason,
                    a.approval_status, a.approval_id, a.request_id, a.session_id, a.masked,
                    a.input_json, a.output_json, a.latency_ms, a.input_tokens, a.output_tokens,
                    a.cost_micros, a.error, a.prompt, a.injection,
                    i.prev_hash, i.entry_hash
             FROM audit_log a
             LEFT JOIN audit_integrity i ON i.audit_id = a.id
             ORDER BY a.id",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut prev = String::new();
        for row in rows {
            let e = entry_from_row(&row)?;
            let stored_prev: Option<String> = row.try_get("prev_hash")?;
            let stored: Option<String> = row.try_get("entry_hash")?;
            let stored_prev = stored_prev.unwrap_or_default();
            let stored = stored.unwrap_or_default();
            if stored.is_empty() || stored_prev != prev || stored != super::integrity_hash(&prev, &e) {
                bail!("audit integrity violation at id {}", e.id);
            }
            prev = stored;
        }
        Ok(())
    }
}

impl Store for PostgresLogger {}

#[async_trait::async_trait]
impl Recorder for PostgresLogger {
    async fn log(&self, e: &Entry) -> Result<()> {
        let mut e = e.clone();
        e.timestamp = clock::truncate_to_second(e.timestamp);

        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;

        // 여러 인스턴스가 동시에 체인 꼬리를 읽고 쓰면 체인이 갈라집니다.
        sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(CHAIN_LOCK).execute(&mut *tx).await?;

        let id: i64 = sqlx::query(
            "INSERT INTO audit_log
                (ts, actor, tool, system, access, decision, reason, approval_status, approval_id,
                 request_id, session_id, input_tokens, output_tokens, cost_micros, masked,
                 input_json, output_json, latency_ms, error, prompt, injection)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)
             RETURNING id",
        )
        .bind(clock::to_rfc3339(e.timestamp))
        .bind(&e.actor)
        .bind(&e.tool)
        .bind(&e.system)
        .bind(&e.access)
        .bind(&e.decision)
        .bind(&e.reason)
        .bind(&e.approval_status)
        .bind(&e.approval_id)
        .bind(&e.request_id)
        .bind(&e.session_id)
        .bind(e.input_tokens)
        .bind(e.output_tokens)
        .bind(e.cost_micros)
        .bind(e.masked as i32)
        .bind(to_json(&e.input))
        .bind(to_json(&e.output))
        .bind(e.latency_ms)
        .bind(&e.error)
        .bind(&e.prompt)
        .bind(&e.injection)
        .fetch_one(&mut *tx)
        .await?
        .try_get("id")?;

        e.id = id;

        let prev: String = sqlx::query("SELECT entry_hash FROM audit_integrity ORDER BY audit_id DESC LIMIT 1")
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| r.try_get::<String, _>("entry_hash"))
            .transpose()?
            .unwrap_or_default();

        let h = super::integrity_hash(&prev, &e);
        sqlx::query("INSERT INTO audit_integrity(audit_id, prev_hash, entry_hash) VALUES ($1,$2,$3)")
            .bind(id)
            .bind(&prev)
            .bind(&h)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }
}

// SQL 문자열을 동적으로 조립하지만 **사용자 데이터는 전부 bind 파라미터로만**
// 들어갑니다. 보간되는 조각은 (a) `GroupBy::column()` — 닫힌 열거형에서만
// 나오는 고정 컬럼명과 (b) 우리가 만든 플레이스홀더(`?` / `$n`) 뿐입니다.
// 그래서 AssertSqlSafe 가 타당합니다.
#[async_trait::async_trait]
impl Reader for PostgresLogger {
    async fn query(&self, f: &Filter) -> Result<Vec<Entry>> {
        let mut sql = String::from(
            "SELECT id, ts, actor, tool, system, access, decision, reason, approval_status,
                    approval_id, request_id, session_id, masked, input_json, output_json,
                    latency_ms, input_tokens, output_tokens, cost_micros, error, prompt, injection
             FROM audit_log WHERE 1=1",
        );
        let mut n = 0;
        let mut next = || {
            n += 1;
            format!("${n}")
        };
        let mut binds: Vec<String> = Vec::new();

        if !f.actor.is_empty() {
            sql.push_str(&format!(" AND actor = {}", next()));
            binds.push(f.actor.clone());
        }
        if !f.tool.is_empty() {
            sql.push_str(&format!(" AND tool = {}", next()));
            binds.push(f.tool.clone());
        }
        if !f.system.is_empty() {
            sql.push_str(&format!(" AND system = {}", next()));
            binds.push(f.system.clone());
        }
        if !f.session_id.is_empty() {
            sql.push_str(&format!(" AND session_id = {}", next()));
            binds.push(f.session_id.clone());
        }
        if !f.decision.is_empty() {
            sql.push_str(&format!(" AND decision = {}", next()));
            binds.push(f.decision.clone());
        }
        if f.errors_only {
            sql.push_str(" AND error IS NOT NULL AND error != ''");
        }
        if f.masked_only {
            sql.push_str(" AND masked = 1");
        }
        if f.injection_only {
            sql.push_str(" AND injection IS NOT NULL AND injection != ''");
        }
        if let Some(since) = f.since {
            sql.push_str(&format!(" AND ts >= {}", next()));
            binds.push(clock::to_rfc3339(since));
        }
        sql.push_str(&format!(" ORDER BY id DESC LIMIT {}", next()));

        let mut q = sqlx::query(AssertSqlSafe(sql.clone()));
        for b in &binds {
            q = q.bind(b);
        }
        q = q.bind(if f.limit <= 0 { 50 } else { f.limit });

        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(entry_from_row).collect()
    }

    async fn recent(&self, limit: i64) -> Result<Vec<Entry>> {
        self.query(&Filter {
            limit,
            ..Default::default()
        })
        .await
    }

    async fn stats(&self, by: GroupBy, since: Option<DateTime<Utc>>) -> Result<Vec<Stat>> {
        let col = by.column();
        let mut sql = format!(
            "SELECT {col} AS k,
                    COUNT(*)::bigint AS calls,
                    COALESCE(SUM(CASE WHEN decision = 'denied' THEN 1 ELSE 0 END),0)::bigint AS denied,
                    COALESCE(SUM(CASE WHEN error IS NOT NULL AND error != '' THEN 1 ELSE 0 END),0)::bigint AS errors,
                    COALESCE(AVG(latency_ms),0)::float8 AS avg_latency,
                    COALESCE(MAX(latency_ms),0)::bigint AS max_latency,
                    COALESCE(SUM(cost_micros),0)::bigint AS cost
             FROM audit_log"
        );
        if since.is_some() {
            sql.push_str(" WHERE ts >= $1");
        }
        sql.push_str(" GROUP BY k ORDER BY calls DESC");

        let mut q = sqlx::query(AssertSqlSafe(sql.clone()));
        if let Some(s) = since {
            q = q.bind(clock::to_rfc3339(s));
        }
        let rows = q.fetch_all(&self.pool).await?;

        Ok(rows
            .iter()
            .map(|r| {
                let key: Option<String> = r.try_get("k").ok().flatten();
                Stat {
                    key: key.filter(|k| !k.is_empty()).unwrap_or_else(|| "(none)".into()),
                    calls: r.try_get("calls").unwrap_or(0),
                    denied: r.try_get("denied").unwrap_or(0),
                    errors: r.try_get("errors").unwrap_or(0),
                    avg_latency_ms: r.try_get("avg_latency").unwrap_or(0.0),
                    max_latency_ms: r.try_get("max_latency").unwrap_or(0),
                    cost_micros: r.try_get("cost").unwrap_or(0),
                }
            })
            .collect())
    }

    async fn oldest(&self) -> Result<HashMap<String, DateTime<Utc>>> {
        let rows = sqlx::query("SELECT tool, MIN(ts) AS first_ts FROM audit_log GROUP BY tool")
            .fetch_all(&self.pool)
            .await?;
        let mut out = HashMap::new();
        for r in rows {
            let tool: String = r.try_get("tool")?;
            let ts: Option<String> = r.try_get("first_ts")?;
            if let Some(ts) = ts
                && let Some(t) = clock::parse_rfc3339(&ts)
            {
                out.insert(tool, t);
            }
        }
        Ok(out)
    }
}

#[async_trait::async_trait]
impl Purger for PostgresLogger {
    async fn purge(&self, p: &Policy, now: DateTime<Utc>, exp: &dyn Exporter) -> Result<Purged> { retention::purge(self, p, now, exp).await }
}

#[async_trait::async_trait]
impl PurgeBackend for PostgresLogger {
    async fn select_for_archive(&self, tool: Option<&str>, exclude_tools: &[String], before: DateTime<Utc>, limit: i64) -> Result<Vec<Entry>> {
        let mut sql = String::from(
            "SELECT id, ts, actor, tool, system, access, decision, reason, approval_status,
                    approval_id, request_id, session_id, masked, input_json, output_json,
                    latency_ms, input_tokens, output_tokens, cost_micros, error, prompt, injection
             FROM audit_log WHERE ts < $1",
        );
        let mut n = 1;
        if tool.is_some() {
            n += 1;
            sql.push_str(&format!(" AND tool = ${n}"));
        }
        if !exclude_tools.is_empty() {
            let marks: Vec<String> = exclude_tools
                .iter()
                .map(|_| {
                    n += 1;
                    format!("${n}")
                })
                .collect();
            sql.push_str(&format!(" AND tool NOT IN ({})", marks.join(",")));
        }
        n += 1;
        sql.push_str(&format!(" ORDER BY id LIMIT ${n}"));

        let mut q = sqlx::query(AssertSqlSafe(sql.clone())).bind(clock::to_rfc3339(before));
        if let Some(t) = tool {
            q = q.bind(t.to_string());
        }
        for t in exclude_tools {
            q = q.bind(t.clone());
        }
        q = q.bind(limit);

        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(entry_from_row).collect()
    }

    async fn delete_by_ids(&self, ids: &[i64]) -> Result<i64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let res = sqlx::query("DELETE FROM audit_log WHERE id = ANY($1)").bind(ids).execute(&self.pool).await?;
        Ok(res.rows_affected() as i64)
    }
}

fn to_json(m: &Option<Map<String, Value>>) -> Option<String> { m.as_ref().map(|m| serde_json::to_string(&Value::Object(m.clone())).unwrap_or_default()) }

fn from_json(s: Option<String>) -> Option<Map<String, Value>> {
    let s = s?;
    if s.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(&s).ok().and_then(|v| v.as_object().cloned())
}

fn entry_from_row(r: &sqlx::postgres::PgRow) -> Result<Entry> {
    let ts: String = r.try_get("ts")?;
    Ok(Entry {
        id: r.try_get("id")?,
        timestamp: clock::parse_rfc3339(&ts).ok_or_else(|| anyhow!("audit: bad timestamp {ts:?}"))?,
        actor: r.try_get("actor")?,
        tool: r.try_get("tool")?,
        system: r.try_get("system")?,
        access: r.try_get("access")?,
        decision: r.try_get("decision")?,
        reason: r.try_get::<Option<String>, _>("reason")?.unwrap_or_default(),
        approval_status: r.try_get::<Option<String>, _>("approval_status")?.unwrap_or_default(),
        approval_id: r.try_get::<Option<String>, _>("approval_id")?.unwrap_or_default(),
        request_id: r.try_get::<Option<String>, _>("request_id")?.unwrap_or_default(),
        session_id: r.try_get::<Option<String>, _>("session_id")?.unwrap_or_default(),
        masked: r.try_get::<i32, _>("masked")? != 0,
        input: from_json(r.try_get("input_json")?),
        output: from_json(r.try_get("output_json")?),
        latency_ms: r.try_get("latency_ms")?,
        input_tokens: r.try_get("input_tokens")?,
        output_tokens: r.try_get("output_tokens")?,
        cost_micros: r.try_get("cost_micros")?,
        error: r.try_get::<Option<String>, _>("error")?.unwrap_or_default(),
        prompt: r.try_get::<Option<String>, _>("prompt")?.unwrap_or_default(),
        injection: r.try_get::<Option<String>, _>("injection")?.unwrap_or_default(),
    })
}
