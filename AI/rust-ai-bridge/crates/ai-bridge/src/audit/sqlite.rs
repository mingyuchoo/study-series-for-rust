//! SQLite 감사 로거 (단일 노드).

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
           Row,
           Sqlite,
           Transaction,
           sqlite::{SqliteConnectOptions,
                    SqliteJournalMode,
                    SqlitePool,
                    SqlitePoolOptions}};
use std::{collections::HashMap,
          path::Path};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS audit_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
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
    input_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens   INTEGER NOT NULL DEFAULT 0,
    cost_micros     INTEGER NOT NULL DEFAULT 0,
    masked          INTEGER NOT NULL DEFAULT 0,
    input_json      TEXT,
    output_json     TEXT,
    latency_ms      INTEGER NOT NULL DEFAULT 0,
    error           TEXT,
    prompt          TEXT,
    injection       TEXT
);
CREATE INDEX IF NOT EXISTS idx_audit_actor ON audit_log(actor);
CREATE INDEX IF NOT EXISTS idx_audit_tool ON audit_log(tool);
CREATE INDEX IF NOT EXISTS idx_audit_ts ON audit_log(ts);
CREATE INDEX IF NOT EXISTS idx_audit_session ON audit_log(session_id);

-- 해시 체인. audit_log 와 1:1 이며, 삭제는 함께 이루어집니다.
CREATE TABLE IF NOT EXISTS audit_integrity (
    audit_id   INTEGER PRIMARY KEY,
    prev_hash  TEXT NOT NULL,
    entry_hash TEXT NOT NULL,
    FOREIGN KEY(audit_id) REFERENCES audit_log(id) ON DELETE CASCADE
);
"#;

/// SQLite 감사 로거.
#[derive(Debug, Clone)]
pub struct SqliteLogger {
    pool: SqlitePool,
}

impl SqliteLogger {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5))
            .foreign_keys(true);
        // 해시 체인의 꼬리를 읽고 쓰는 구간이 직렬화되어야 하므로 연결을 하나로 둡니다.
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
        Self::from_pool(pool).await
    }

    pub async fn open_in_memory() -> Result<Self> {
        let opts = SqliteConnectOptions::new().in_memory(true).foreign_keys(true);
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
        Self::from_pool(pool).await
    }

    pub async fn from_pool(pool: SqlitePool) -> Result<Self> {
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

    pub fn pool(&self) -> &SqlitePool { &self.pool }

    /// 해시 체인을 처음부터 끝까지 검증합니다.
    ///
    /// 행 삭제·수정·체인 단절을 모두 잡아냅니다.
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

impl Store for SqliteLogger {}

#[async_trait::async_trait]
impl Recorder for SqliteLogger {
    async fn log(&self, e: &Entry) -> Result<()> {
        let mut e = e.clone();
        // 저장 형식과 해시 입력을 같게 정규화합니다.
        e.timestamp = clock::truncate_to_second(e.timestamp);

        let mut tx: Transaction<'_, Sqlite> = self.pool.begin().await?;

        let id: i64 = sqlx::query(
            "INSERT INTO audit_log
                (ts, actor, tool, system, access, decision, reason, approval_status, approval_id,
                 request_id, session_id, input_tokens, output_tokens, cost_micros, masked,
                 input_json, output_json, latency_ms, error, prompt, injection)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
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
        .bind(e.masked as i64)
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

        // 체인 꼬리를 읽어 새 해시를 잇습니다. 같은 트랜잭션 안이라 끊긴 고리가 생기지
        // 않습니다.
        let prev: String = sqlx::query("SELECT entry_hash FROM audit_integrity ORDER BY audit_id DESC LIMIT 1")
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| r.try_get::<String, _>("entry_hash"))
            .transpose()?
            .unwrap_or_default();

        let h = super::integrity_hash(&prev, &e);
        sqlx::query("INSERT INTO audit_integrity(audit_id, prev_hash, entry_hash) VALUES (?,?,?)")
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
impl Reader for SqliteLogger {
    async fn query(&self, f: &Filter) -> Result<Vec<Entry>> {
        let mut sql = String::from(
            "SELECT id, ts, actor, tool, system, access, decision, reason, approval_status,
                    approval_id, request_id, session_id, masked, input_json, output_json,
                    latency_ms, input_tokens, output_tokens, cost_micros, error, prompt, injection
             FROM audit_log WHERE 1=1",
        );
        if !f.actor.is_empty() {
            sql.push_str(" AND actor = ?");
        }
        if !f.tool.is_empty() {
            sql.push_str(" AND tool = ?");
        }
        if !f.system.is_empty() {
            sql.push_str(" AND system = ?");
        }
        if !f.session_id.is_empty() {
            sql.push_str(" AND session_id = ?");
        }
        if !f.decision.is_empty() {
            sql.push_str(" AND decision = ?");
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
        if f.since.is_some() {
            sql.push_str(" AND ts >= ?");
        }
        sql.push_str(" ORDER BY id DESC LIMIT ?");

        let mut q = sqlx::query(AssertSqlSafe(sql.clone()));
        if !f.actor.is_empty() {
            q = q.bind(&f.actor);
        }
        if !f.tool.is_empty() {
            q = q.bind(&f.tool);
        }
        if !f.system.is_empty() {
            q = q.bind(&f.system);
        }
        if !f.session_id.is_empty() {
            q = q.bind(&f.session_id);
        }
        if !f.decision.is_empty() {
            q = q.bind(&f.decision);
        }
        if let Some(since) = f.since {
            q = q.bind(clock::to_rfc3339(since));
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
        // `by.column()` 은 열거형에서만 나오므로 문자열 보간이 안전합니다.
        let col = by.column();
        let mut sql = format!(
            "SELECT {col} AS k,
                    COUNT(*) AS calls,
                    SUM(CASE WHEN decision = 'denied' THEN 1 ELSE 0 END) AS denied,
                    SUM(CASE WHEN error IS NOT NULL AND error != '' THEN 1 ELSE 0 END) AS errors,
                    AVG(latency_ms) AS avg_latency,
                    MAX(latency_ms) AS max_latency,
                    SUM(cost_micros) AS cost
             FROM audit_log"
        );
        if since.is_some() {
            sql.push_str(" WHERE ts >= ?");
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
impl Purger for SqliteLogger {
    async fn purge(&self, p: &Policy, now: DateTime<Utc>, exp: &dyn Exporter) -> Result<Purged> { retention::purge(self, p, now, exp).await }
}

#[async_trait::async_trait]
impl PurgeBackend for SqliteLogger {
    async fn select_for_archive(&self, tool: Option<&str>, exclude_tools: &[String], before: DateTime<Utc>, limit: i64) -> Result<Vec<Entry>> {
        let mut sql = String::from(
            "SELECT id, ts, actor, tool, system, access, decision, reason, approval_status,
                    approval_id, request_id, session_id, masked, input_json, output_json,
                    latency_ms, input_tokens, output_tokens, cost_micros, error, prompt, injection
             FROM audit_log WHERE ts < ?",
        );
        if tool.is_some() {
            sql.push_str(" AND tool = ?");
        }
        if !exclude_tools.is_empty() {
            let marks = vec!["?"; exclude_tools.len()].join(",");
            sql.push_str(&format!(" AND tool NOT IN ({marks})"));
        }
        sql.push_str(" ORDER BY id LIMIT ?");

        let mut q = sqlx::query(AssertSqlSafe(sql.clone())).bind(clock::to_rfc3339(before));
        if let Some(t) = tool {
            q = q.bind(t);
        }
        for t in exclude_tools {
            q = q.bind(t);
        }
        q = q.bind(limit);

        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(entry_from_row).collect()
    }

    async fn delete_by_ids(&self, ids: &[i64]) -> Result<i64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let marks = vec!["?"; ids.len()].join(",");
        // 무결성 행은 ON DELETE CASCADE 로 함께 사라집니다.
        let mut q = sqlx::query(AssertSqlSafe(format!("DELETE FROM audit_log WHERE id IN ({marks})")));
        for id in ids {
            q = q.bind(id);
        }
        Ok(q.execute(&self.pool).await?.rows_affected() as i64)
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

fn entry_from_row(r: &sqlx::sqlite::SqliteRow) -> Result<Entry> {
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
        masked: r.try_get::<i64, _>("masked")? != 0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::archive::{Discard,
                                FailingExporter,
                                RecordingExporter};
    use chrono::{Duration,
                 TimeZone};
    use serde_json::json;

    async fn logger() -> SqliteLogger { SqliteLogger::open_in_memory().await.unwrap() }

    fn epoch() -> DateTime<Utc> { Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap() }

    fn entry(tool: &str, at: DateTime<Utc>) -> Entry {
        Entry {
            timestamp: at,
            actor: "emp-sales-01".into(),
            tool: tool.into(),
            system: "erp".into(),
            access: "read".into(),
            decision: "allowed".into(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn logs_and_reads_back() {
        let l = logger().await;
        let mut e = entry("get_invoice_status", epoch());
        e.prompt = "INV-1 결제됐어?".into();
        e.cost_micros = 4200;
        e.masked = true;
        e.input = Some(json!({"invoice_id":"INV-1"}).as_object().unwrap().clone());
        l.log(&e).await.unwrap();

        let got = l.recent(10).await.unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].prompt, "INV-1 결제됐어?");
        assert_eq!(got[0].cost_micros, 4200);
        assert!(got[0].masked);
        assert_eq!(got[0].input.as_ref().unwrap()["invoice_id"], json!("INV-1"));
    }

    #[tokio::test]
    async fn hash_chain_verifies() {
        let l = logger().await;
        for i in 0 .. 5 {
            l.log(&entry("t", epoch() + Duration::seconds(i))).await.unwrap();
        }
        l.verify_integrity().await.unwrap();
    }

    #[tokio::test]
    async fn tampering_with_a_row_breaks_the_chain() {
        let l = logger().await;
        for i in 0 .. 3 {
            l.log(&entry("t", epoch() + Duration::seconds(i))).await.unwrap();
        }
        l.verify_integrity().await.unwrap();

        // 감사 로그는 UPDATE 를 노출하지 않지만, DB 를 직접 만진 상황을 흉내냅니다.
        sqlx::query("UPDATE audit_log SET decision = 'allowed' WHERE id = 2")
            .execute(l.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE audit_log SET actor = 'attacker' WHERE id = 2")
            .execute(l.pool())
            .await
            .unwrap();

        let err = l.verify_integrity().await.unwrap_err();
        assert!(err.to_string().contains("integrity violation at id 2"));
    }

    #[tokio::test]
    async fn deleting_a_row_breaks_the_chain() {
        let l = logger().await;
        for i in 0 .. 3 {
            l.log(&entry("t", epoch() + Duration::seconds(i))).await.unwrap();
        }
        sqlx::query("DELETE FROM audit_log WHERE id = 2").execute(l.pool()).await.unwrap();
        assert!(l.verify_integrity().await.is_err());
    }

    #[tokio::test]
    async fn filters_narrow_independently() {
        let l = logger().await;
        let mut a = entry("t1", epoch());
        a.actor = "a".into();
        l.log(&a).await.unwrap();

        let mut b = entry("t2", epoch());
        b.actor = "a".into();
        b.decision = "denied".into();
        l.log(&b).await.unwrap();

        let mut c = entry("t2", epoch());
        c.actor = "a".into();
        c.error = "boom".into();
        l.log(&c).await.unwrap();

        let mut d = entry("t3", epoch());
        d.actor = "b".into();
        d.masked = true;
        l.log(&d).await.unwrap();

        async fn n(l: &SqliteLogger, f: Filter) -> usize { l.query(&f).await.unwrap().len() }
        assert_eq!(
            n(
                &l,
                Filter {
                    actor: "a".into(),
                    ..Default::default()
                }
            )
            .await,
            3
        );
        assert_eq!(
            n(
                &l,
                Filter {
                    tool: "t2".into(),
                    ..Default::default()
                }
            )
            .await,
            2
        );
        assert_eq!(
            n(
                &l,
                Filter {
                    decision: "denied".into(),
                    ..Default::default()
                }
            )
            .await,
            1
        );
        assert_eq!(
            n(
                &l,
                Filter {
                    errors_only: true,
                    ..Default::default()
                }
            )
            .await,
            1
        );
        assert_eq!(
            n(
                &l,
                Filter {
                    masked_only: true,
                    ..Default::default()
                }
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn stats_aggregate_calls_denials_and_cost() {
        let l = logger().await;
        let mut a = entry("t", epoch());
        a.cost_micros = 100;
        l.log(&a).await.unwrap();

        let mut b = entry("t", epoch());
        b.decision = "denied".into();
        b.cost_micros = 200;
        l.log(&b).await.unwrap();

        let st = l.stats(GroupBy::Actor, None).await.unwrap();
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].calls, 2);
        assert_eq!(st[0].denied, 1);
        assert_eq!(st[0].cost_micros, 300);
    }

    #[tokio::test]
    async fn oldest_reports_the_earliest_record_per_tool() {
        let l = logger().await;
        l.log(&entry("a", epoch() - Duration::days(10))).await.unwrap();
        l.log(&entry("a", epoch())).await.unwrap();

        let o = l.oldest().await.unwrap();
        assert_eq!(o["a"], epoch() - Duration::days(10));
    }

    // --- 보존 기간 ---

    #[tokio::test]
    async fn export_failure_deletes_nothing() {
        // 이것이 핵심 계약입니다.
        let l = logger().await;
        l.log(&entry("t", epoch() - Duration::days(100))).await.unwrap();

        let p = Policy {
            by_tool: HashMap::from([("t".to_string(), 30)]),
            default: 0,
        };
        let res = l.purge(&p, epoch(), &FailingExporter).await;
        assert!(res.is_err());
        // 기록이 그대로 남아 있어야 합니다.
        assert_eq!(l.recent(10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn archives_then_deletes_only_expired_records() {
        let l = logger().await;
        l.log(&entry("t", epoch() - Duration::days(100))).await.unwrap();
        l.log(&entry("t", epoch())).await.unwrap();

        let exp = RecordingExporter::default();
        let p = Policy {
            by_tool: HashMap::from([("t".to_string(), 30)]),
            default: 0,
        };
        let out = l.purge(&p, epoch(), &exp).await.unwrap();

        assert_eq!(out.deleted, 1);
        // 내보낸 건수와 지운 건수가 같아야 합니다.
        assert_eq!(exp.exported.lock().unwrap().len(), 1);
        // 최근 기록은 살아 있습니다.
        assert_eq!(l.recent(10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn retention_zero_keeps_forever() {
        // 보존 0 = 영구 보존. 도구 이름을 바꾼 것만으로 기록이 사라지면 안 됩니다.
        let l = logger().await;
        l.log(&entry("t", epoch() - Duration::days(365 * 5))).await.unwrap();

        let p = Policy {
            by_tool: HashMap::from([("t".to_string(), 0)]),
            default: 0,
        };
        let out = l.purge(&p, epoch(), &Discard).await.unwrap();
        assert_eq!(out.deleted, 0);
        assert_eq!(out.skipped, vec!["t"]);
        assert_eq!(l.recent(10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn default_retention_skips_explicitly_listed_tools() {
        let l = logger().await;
        l.log(&entry("listed", epoch() - Duration::days(100))).await.unwrap();
        l.log(&entry("unlisted", epoch() - Duration::days(100))).await.unwrap();

        // listed 는 0(영구)으로 명시되었으므로 기본 정책이 건드리면 안 됩니다.
        let p = Policy {
            by_tool: HashMap::from([("listed".to_string(), 0)]),
            default: 30,
        };
        let out = l.purge(&p, epoch(), &Discard).await.unwrap();
        assert_eq!(out.deleted, 1);

        let left = l.recent(10).await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].tool, "listed");
    }

    #[tokio::test]
    async fn purge_across_multiple_batches() {
        let l = logger().await;
        // 배치 크기(500)를 넘겨 루프가 도는지 확인합니다.
        for i in 0 .. (retention::PURGE_BATCH + 20) {
            l.log(&entry("t", epoch() - Duration::days(100) + Duration::seconds(i))).await.unwrap();
        }
        let exp = RecordingExporter::default();
        let p = Policy {
            by_tool: HashMap::from([("t".to_string(), 30)]),
            default: 0,
        };
        let out = l.purge(&p, epoch(), &exp).await.unwrap();
        assert_eq!(out.deleted, retention::PURGE_BATCH + 20);
        assert_eq!(exp.exported.lock().unwrap().len() as i64, retention::PURGE_BATCH + 20);
        assert_eq!(l.recent(10).await.unwrap().len(), 0);
    }
}
