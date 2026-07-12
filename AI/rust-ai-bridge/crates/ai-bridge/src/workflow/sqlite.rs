//! SQLite 워크플로 저장소.
//!
//! `save` 는 **하나의 UPSERT 문**으로 낙관적 잠금을 겁니다 — `SELECT` 뒤
//! `UPDATE` 로 나누면 그 사이에 다른 인스턴스가 끼어들 수 있습니다(TOCTOU).

use super::{Error,
            Event,
            Run,
            Status,
            Store,
            completed_from_json,
            completed_to_json,
            values_from_json,
            values_to_json};
use crate::clock;
use anyhow::anyhow;
use sqlx::{AssertSqlSafe,
           Row,
           sqlite::{SqliteConnectOptions,
                    SqliteJournalMode,
                    SqlitePool,
                    SqlitePoolOptions}};
use std::{path::Path,
          time::Duration};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS workflow_run (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    status           TEXT NOT NULL,
    completed_json   TEXT NOT NULL,
    values_json      TEXT NOT NULL,
    error            TEXT,
    compensate_error TEXT,
    started_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    version          INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_workflow_status ON workflow_run(status);
CREATE INDEX IF NOT EXISTS idx_workflow_name ON workflow_run(name);

CREATE TABLE IF NOT EXISTS workflow_event (
    seq           INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id        TEXT NOT NULL,
    ts            TEXT NOT NULL,
    type          TEXT NOT NULL,
    step          TEXT,
    attempt       INTEGER NOT NULL DEFAULT 0,
    worker        TEXT,
    fencing_token INTEGER NOT NULL DEFAULT 0,
    message       TEXT
);
CREATE INDEX IF NOT EXISTS idx_workflow_event_run ON workflow_event(run_id, seq);
"#;

const COLS: &str = "id, name, status, completed_json, values_json, error, compensate_error,
                    started_at, updated_at, version";

#[derive(Debug, Clone)]
pub struct SqliteWorkflowStore {
    pool: SqlitePool,
}

impl SqliteWorkflowStore {
    pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?;
        Self::from_pool(pool).await
    }

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
        })
    }

    pub fn pool(&self) -> &SqlitePool { &self.pool }
}

#[async_trait::async_trait]
impl Store for SqliteWorkflowStore {
    async fn load(&self, run_id: &str) -> Result<Option<Run>, Error> {
        let row = sqlx::query(AssertSqlSafe(format!("SELECT {COLS} FROM workflow_run WHERE id = ?")))
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(sql)?;
        row.as_ref().map(run_from_row).transpose()
    }

    async fn save(&self, run: &Run) -> Result<Run, Error> {
        // 새 행이면 ON CONFLICT 가 발동하지 않아 그대로 들어가고, 기존 행이면
        // **저장된 version 이 호출자가 읽은 version 과 같을 때만** 갱신됩니다.
        let res = sqlx::query(
            "INSERT INTO workflow_run
                (id, name, status, completed_json, values_json, error, compensate_error,
                 started_at, updated_at, version)
             VALUES (?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET
                status=excluded.status,
                completed_json=excluded.completed_json,
                values_json=excluded.values_json,
                error=excluded.error,
                compensate_error=excluded.compensate_error,
                updated_at=excluded.updated_at,
                version=version+1
             WHERE version = ?",
        )
        .bind(&run.id)
        .bind(&run.name)
        .bind(run.status.as_str())
        .bind(completed_to_json(&run.completed))
        .bind(values_to_json(&run.values))
        .bind(&run.error)
        .bind(&run.compensate_error)
        .bind(clock::to_rfc3339_nanos(run.started_at.unwrap_or_else(chrono::Utc::now)))
        .bind(clock::to_rfc3339_nanos(run.updated_at.unwrap_or_else(chrono::Utc::now)))
        .bind(run.version + 1)
        .bind(run.version)
        .execute(&self.pool)
        .await
        .map_err(sql)?;

        if res.rows_affected() == 0 {
            // 다른 인스턴스가 먼저 저장했습니다 — 덮어쓰면 완료 단계가 뒤로 돌아갑니다.
            return Err(Error::VersionConflict);
        }

        let mut saved = run.clone();
        saved.version += 1;
        Ok(saved)
    }

    async fn list(&self, status: Option<Status>, limit: i64) -> Result<Vec<Run>, Error> {
        let limit = if limit <= 0 { 50 } else { limit };
        let rows = match status {
            | Some(s) =>
                sqlx::query(AssertSqlSafe(format!(
                    "SELECT {COLS} FROM workflow_run WHERE status = ?
                     ORDER BY updated_at DESC LIMIT ?"
                )))
                .bind(s.as_str())
                .bind(limit)
                .fetch_all(&self.pool)
                .await,
            | None =>
                sqlx::query(AssertSqlSafe(format!("SELECT {COLS} FROM workflow_run ORDER BY updated_at DESC LIMIT ?")))
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await,
        }
        .map_err(sql)?;
        rows.iter().map(run_from_row).collect()
    }

    async fn append_event(&self, e: &Event) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO workflow_event
                (run_id, ts, type, step, attempt, worker, fencing_token, message)
             VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(&e.run_id)
        .bind(clock::to_rfc3339_nanos(e.at))
        .bind(&e.r#type)
        .bind(&e.step)
        .bind(e.attempt)
        .bind(&e.worker)
        .bind(e.fencing_token)
        .bind(&e.message)
        .execute(&self.pool)
        .await
        .map_err(sql)?;
        Ok(())
    }

    async fn events(&self, run_id: &str) -> Result<Vec<Event>, Error> {
        let rows = sqlx::query(
            "SELECT run_id, ts, type, step, attempt, worker, fencing_token, message
             FROM workflow_event WHERE run_id = ? ORDER BY seq",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(sql)?;

        rows.iter()
            .map(|r| {
                let ts: String = r.try_get("ts").map_err(sql)?;
                Ok(Event {
                    run_id: r.try_get("run_id").map_err(sql)?,
                    at: clock::parse_rfc3339(&ts).ok_or_else(|| Error::Other(anyhow!("workflow: bad event ts")))?,
                    r#type: r.try_get("type").map_err(sql)?,
                    step: r.try_get::<Option<String>, _>("step").map_err(sql)?.unwrap_or_default(),
                    attempt: r.try_get("attempt").map_err(sql)?,
                    worker: r.try_get::<Option<String>, _>("worker").map_err(sql)?.unwrap_or_default(),
                    fencing_token: r.try_get("fencing_token").map_err(sql)?,
                    message: r.try_get::<Option<String>, _>("message").map_err(sql)?.unwrap_or_default(),
                })
            })
            .collect()
    }
}

fn sql(e: impl std::fmt::Display) -> Error { Error::Other(anyhow!("workflow: {e}")) }

fn run_from_row(r: &sqlx::sqlite::SqliteRow) -> Result<Run, Error> {
    let status: String = r.try_get("status").map_err(sql)?;
    let started_at: String = r.try_get("started_at").map_err(sql)?;
    let updated_at: String = r.try_get("updated_at").map_err(sql)?;
    let values_json: String = r.try_get("values_json").map_err(sql)?;
    let completed_json: String = r.try_get("completed_json").map_err(sql)?;

    let mut run = Run {
        id: r.try_get("id").map_err(sql)?,
        name: r.try_get("name").map_err(sql)?,
        status: Status::parse(&status).ok_or_else(|| Error::Other(anyhow!("workflow: unknown status {status:?}")))?,
        completed: completed_from_json(&completed_json),
        values: values_from_json(&values_json),
        error: r.try_get::<Option<String>, _>("error").map_err(sql)?.unwrap_or_default(),
        compensate_error: r.try_get::<Option<String>, _>("compensate_error").map_err(sql)?.unwrap_or_default(),
        started_at: clock::parse_rfc3339(&started_at),
        updated_at: clock::parse_rfc3339(&updated_at),
        version: r.try_get("version").map_err(sql)?,
        ..Default::default()
    };
    // 메타데이터는 values_json 안에 있습니다.
    super::hydrate_metadata(&mut run);
    Ok(run)
}
