//! SQLite 평가 저장소.
//!
//! 턴 스냅샷은 **불변**이고, 레이팅은 **append-only** 입니다. `eval_run` 헤더만
//! 예외적으로 UPDATE 합니다(결과 row 는 여전히 append-only).

use super::{Error,
            GroupBy,
            KNOWN_LABELS,
            Rating,
            RatingFilter,
            Run,
            RunResult,
            Stat,
            StatFilter,
            Store,
            ToolStep,
            Turn,
            TurnFilter,
            compute_content_hash,
            new_id,
            validate_rating,
            validate_turn};
use crate::clock;
use anyhow::anyhow;
use chrono::{DateTime,
             Utc};
use serde_json::Value;
use sqlx::{AssertSqlSafe,
           Row,
           sqlite::{SqliteConnectOptions,
                    SqliteJournalMode,
                    SqlitePool,
                    SqlitePoolOptions}};
use std::path::Path;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS eval_turn (
    turn_id         TEXT PRIMARY KEY,
    ts              TEXT NOT NULL,
    session_id      TEXT NOT NULL DEFAULT '',
    actor           TEXT NOT NULL DEFAULT '',
    agent_id        TEXT NOT NULL DEFAULT '',
    channel         TEXT NOT NULL DEFAULT '',
    model           TEXT NOT NULL DEFAULT '',
    prompt          TEXT NOT NULL DEFAULT '',
    reply           TEXT NOT NULL DEFAULT '',
    tool_trail_json TEXT NOT NULL DEFAULT '[]',
    outcome         TEXT NOT NULL DEFAULT '',
    input_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens   INTEGER NOT NULL DEFAULT 0,
    cost_micros     INTEGER NOT NULL DEFAULT 0,
    content_hash    TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_eval_turn_ts ON eval_turn(ts);
CREATE INDEX IF NOT EXISTS idx_eval_turn_actor ON eval_turn(actor);
CREATE INDEX IF NOT EXISTS idx_eval_turn_agent ON eval_turn(agent_id);
CREATE INDEX IF NOT EXISTS idx_eval_turn_session ON eval_turn(session_id);

CREATE TABLE IF NOT EXISTS eval_rating (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    turn_id        TEXT NOT NULL,
    ts             TEXT NOT NULL,
    source         TEXT NOT NULL,
    rater_id       TEXT NOT NULL,
    score          REAL NOT NULL,
    scale          TEXT NOT NULL,
    labels_json    TEXT NOT NULL DEFAULT '[]',
    note           TEXT NOT NULL DEFAULT '',
    rubric_id      TEXT NOT NULL DEFAULT '',
    rubric_version TEXT NOT NULL DEFAULT '',
    dims_json      TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY(turn_id) REFERENCES eval_turn(turn_id)
);
CREATE INDEX IF NOT EXISTS idx_eval_rating_turn ON eval_rating(turn_id);
CREATE INDEX IF NOT EXISTS idx_eval_rating_ts ON eval_rating(ts);
CREATE INDEX IF NOT EXISTS idx_eval_rating_source ON eval_rating(source);

CREATE TABLE IF NOT EXISTS eval_run (
    run_id      TEXT PRIMARY KEY,
    suite       TEXT NOT NULL,
    started_at  TEXT NOT NULL,
    finished_at TEXT NOT NULL DEFAULT '',
    git_sha     TEXT NOT NULL DEFAULT '',
    model       TEXT NOT NULL DEFAULT '',
    pass_count  INTEGER NOT NULL DEFAULT 0,
    fail_count  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS eval_run_result (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id       TEXT NOT NULL,
    case_id      TEXT NOT NULL,
    pass         INTEGER NOT NULL,
    score        REAL NOT NULL DEFAULT 0,
    actual_reply TEXT NOT NULL DEFAULT '',
    trail_json   TEXT NOT NULL DEFAULT '[]',
    error        TEXT NOT NULL DEFAULT '',
    FOREIGN KEY(run_id) REFERENCES eval_run(run_id)
);
CREATE INDEX IF NOT EXISTS idx_eval_run_result_run ON eval_run_result(run_id);
"#;

const TURN_COLS: &str = "turn_id, ts, session_id, actor, agent_id, channel, model, prompt, reply,
                         tool_trail_json, outcome, input_tokens, output_tokens, cost_micros,
                         content_hash";

#[derive(Debug, Clone)]
pub struct SqliteEvalStore {
    pool: SqlitePool,
}

impl SqliteEvalStore {
    pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5));
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
}

#[async_trait::async_trait]
impl Store for SqliteEvalStore {
    async fn record_turn(&self, t: &Turn) -> Result<Turn, Error> {
        validate_turn(t)?;
        let mut t = t.clone();
        if t.turn_id.is_empty() {
            t.turn_id = new_id("turn");
        }
        let ts = clock::truncate_to_second(t.timestamp.unwrap_or_else(Utc::now));
        t.timestamp = Some(ts);
        if t.content_hash.is_empty() {
            t.content_hash = compute_content_hash(&t.prompt, &t.reply, &t.tool_trail);
        }

        let res = sqlx::query(
            "INSERT INTO eval_turn
                (turn_id, ts, session_id, actor, agent_id, channel, model, prompt, reply,
                 tool_trail_json, outcome, input_tokens, output_tokens, cost_micros, content_hash)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(&t.turn_id)
        .bind(clock::to_rfc3339(ts))
        .bind(&t.session_id)
        .bind(&t.actor)
        .bind(&t.agent_id)
        .bind(&t.channel)
        .bind(&t.model)
        .bind(&t.prompt)
        .bind(&t.reply)
        .bind(serde_json::to_string(&t.tool_trail).unwrap_or_else(|_| "[]".into()))
        .bind(&t.outcome)
        .bind(t.input_tokens)
        .bind(t.output_tokens)
        .bind(t.cost_micros)
        .bind(&t.content_hash)
        .execute(&self.pool)
        .await;

        match res {
            | Ok(_) => Ok(t),
            | Err(e) if is_unique_violation(&e) => Err(Error::Invalid(format!(
                // 스냅샷은 불변입니다 — 덮어쓰기가 아니라 오류입니다.
                "turn_id {} already exists",
                t.turn_id
            ))),
            | Err(e) => Err(Error::Other(anyhow!("eval: {e}"))),
        }
    }

    async fn rate(&self, r: &Rating) -> Result<Rating, Error> {
        validate_rating(r)?;
        let mut r = r.clone();
        let ts = clock::truncate_to_second(r.timestamp.unwrap_or_else(Utc::now));
        r.timestamp = Some(ts);

        // 없는 턴에 점수를 붙일 수 없습니다.
        let exists = sqlx::query("SELECT 1 FROM eval_turn WHERE turn_id = ?")
            .bind(&r.turn_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(sql)?;
        if exists.is_none() {
            return Err(Error::NotFound);
        }

        // **UPDATE 경로가 없습니다** — 모든 점수는 새 행입니다.
        let id: i64 = sqlx::query(
            "INSERT INTO eval_rating
                (turn_id, ts, source, rater_id, score, scale, labels_json, note,
                 rubric_id, rubric_version, dims_json)
             VALUES (?,?,?,?,?,?,?,?,?,?,?)
             RETURNING id",
        )
        .bind(&r.turn_id)
        .bind(clock::to_rfc3339(ts))
        .bind(&r.source)
        .bind(&r.rater_id)
        .bind(r.score)
        .bind(&r.scale)
        .bind(serde_json::to_string(&r.labels).unwrap_or_else(|_| "[]".into()))
        .bind(&r.note)
        .bind(&r.rubric_id)
        .bind(&r.rubric_version)
        .bind(serde_json::to_string(&Value::Object(r.dims.clone())).unwrap_or_else(|_| "{}".into()))
        .fetch_one(&self.pool)
        .await
        .map_err(sql)?
        .try_get("id")
        .map_err(sql)?;

        r.id = id;
        Ok(r)
    }

    async fn get_turn(&self, turn_id: &str) -> Result<Turn, Error> {
        let row = sqlx::query(AssertSqlSafe(format!("SELECT {TURN_COLS} FROM eval_turn WHERE turn_id = ?")))
            .bind(turn_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(sql)?
            .ok_or(Error::NotFound)?;
        turn_from_row(&row)
    }

    async fn query_turns(&self, f: &TurnFilter) -> Result<Vec<Turn>, Error> {
        let mut sql_text = format!("SELECT {TURN_COLS} FROM eval_turn t WHERE 1=1");
        if !f.actor.is_empty() {
            sql_text.push_str(" AND actor = ?");
        }
        if !f.agent_id.is_empty() {
            sql_text.push_str(" AND agent_id = ?");
        }
        if !f.session_id.is_empty() {
            sql_text.push_str(" AND session_id = ?");
        }
        if !f.channel.is_empty() {
            sql_text.push_str(" AND channel = ?");
        }
        if !f.outcome.is_empty() {
            sql_text.push_str(" AND outcome = ?");
        }
        if f.unrated_only {
            sql_text.push_str(" AND NOT EXISTS (SELECT 1 FROM eval_rating r WHERE r.turn_id = t.turn_id)");
        }
        if f.since.is_some() {
            sql_text.push_str(" AND ts >= ?");
        }
        if f.until.is_some() {
            sql_text.push_str(" AND ts <= ?");
        }
        sql_text.push_str(" ORDER BY ts DESC LIMIT ?");

        let mut q = sqlx::query(AssertSqlSafe(sql_text));
        for v in [&f.actor, &f.agent_id, &f.session_id, &f.channel, &f.outcome] {
            if !v.is_empty() {
                q = q.bind(v);
            }
        }
        if let Some(s) = f.since {
            q = q.bind(clock::to_rfc3339(s));
        }
        if let Some(u) = f.until {
            q = q.bind(clock::to_rfc3339(u));
        }
        q = q.bind(if f.limit <= 0 { 50 } else { f.limit });

        let rows = q.fetch_all(&self.pool).await.map_err(sql)?;
        rows.iter().map(turn_from_row).collect()
    }

    async fn query_ratings(&self, f: &RatingFilter) -> Result<Vec<Rating>, Error> {
        let mut sql_text = String::from(
            "SELECT id, turn_id, ts, source, rater_id, score, scale, labels_json, note,
                    rubric_id, rubric_version, dims_json
             FROM eval_rating WHERE 1=1",
        );
        if !f.turn_id.is_empty() {
            sql_text.push_str(" AND turn_id = ?");
        }
        if !f.source.is_empty() {
            sql_text.push_str(" AND source = ?");
        }
        if !f.scale.is_empty() {
            sql_text.push_str(" AND scale = ?");
        }
        if !f.rater_id.is_empty() {
            sql_text.push_str(" AND rater_id = ?");
        }
        if !f.label.is_empty() {
            sql_text.push_str(" AND labels_json LIKE ?");
        }
        if f.since.is_some() {
            sql_text.push_str(" AND ts >= ?");
        }
        sql_text.push_str(" ORDER BY id DESC LIMIT ?");

        let mut q = sqlx::query(AssertSqlSafe(sql_text));
        for v in [&f.turn_id, &f.source, &f.scale, &f.rater_id] {
            if !v.is_empty() {
                q = q.bind(v);
            }
        }
        if !f.label.is_empty() {
            q = q.bind(format!("%\"{}\"%", f.label));
        }
        if let Some(s) = f.since {
            q = q.bind(clock::to_rfc3339(s));
        }
        q = q.bind(if f.limit <= 0 { 100 } else { f.limit });

        let rows = q.fetch_all(&self.pool).await.map_err(sql)?;
        rows.iter().map(rating_from_row).collect()
    }

    async fn stats(&self, by: GroupBy, f: &StatFilter) -> Result<Vec<Stat>, Error> {
        match by {
            // 턴 축 — source/scale 필터를 **JOIN 조건**에 둡니다. WHERE 에 두면 점수가 없는
            // 턴이 분모에서도 사라져 "미평가 턴"이 통계에서 증발합니다.
            | GroupBy::Agent | GroupBy::Channel => {
                let col = if by == GroupBy::Agent { "agent_id" } else { "channel" };
                let mut join = String::from("LEFT JOIN eval_rating r ON r.turn_id = t.turn_id");
                if !f.source.is_empty() {
                    join.push_str(" AND r.source = ?");
                }
                if !f.scale.is_empty() {
                    join.push_str(" AND r.scale = ?");
                }
                let mut sql_text = format!(
                    "SELECT t.{col} AS k, COUNT(DISTINCT t.turn_id) AS turns,
                            COUNT(r.id) AS ratings,
                            COALESCE(AVG(r.score),0) AS avg_score,
                            COALESCE(SUM(CASE WHEN r.scale='thumbs' AND r.score>=0.5 THEN 1 ELSE 0 END),0) AS up,
                            COALESCE(SUM(CASE WHEN r.scale='thumbs' AND r.score<0.5 THEN 1 ELSE 0 END),0) AS down
                     FROM eval_turn t {join} WHERE 1=1"
                );
                if f.since.is_some() {
                    sql_text.push_str(" AND t.ts >= ?");
                }
                sql_text.push_str(" GROUP BY k ORDER BY ratings DESC, turns DESC");

                let mut q = sqlx::query(AssertSqlSafe(sql_text));
                if !f.source.is_empty() {
                    q = q.bind(&f.source);
                }
                if !f.scale.is_empty() {
                    q = q.bind(&f.scale);
                }
                if let Some(s) = f.since {
                    q = q.bind(clock::to_rfc3339(s));
                }
                let rows = q.fetch_all(&self.pool).await.map_err(sql)?;
                Ok(rows.iter().map(stat_from_row).collect())
            },

            // 점수 축.
            | GroupBy::Source | GroupBy::Scale => {
                let col = if by == GroupBy::Source { "source" } else { "scale" };
                let mut sql_text = format!(
                    "SELECT {col} AS k, 0 AS turns, COUNT(*) AS ratings,
                            COALESCE(AVG(score),0) AS avg_score,
                            COALESCE(SUM(CASE WHEN scale='thumbs' AND score>=0.5 THEN 1 ELSE 0 END),0) AS up,
                            COALESCE(SUM(CASE WHEN scale='thumbs' AND score<0.5 THEN 1 ELSE 0 END),0) AS down
                     FROM eval_rating WHERE 1=1"
                );
                if !f.source.is_empty() {
                    sql_text.push_str(" AND source = ?");
                }
                if !f.scale.is_empty() {
                    sql_text.push_str(" AND scale = ?");
                }
                if f.since.is_some() {
                    sql_text.push_str(" AND ts >= ?");
                }
                sql_text.push_str(" GROUP BY k ORDER BY ratings DESC");

                let mut q = sqlx::query(AssertSqlSafe(sql_text));
                if !f.source.is_empty() {
                    q = q.bind(&f.source);
                }
                if !f.scale.is_empty() {
                    q = q.bind(&f.scale);
                }
                if let Some(s) = f.since {
                    q = q.bind(clock::to_rfc3339(s));
                }
                let rows = q.fetch_all(&self.pool).await.map_err(sql)?;
                Ok(rows.iter().map(stat_from_row).collect())
            },

            // 라벨은 JSON 배열이라 라벨마다 따로 셉니다.
            | GroupBy::Label => {
                let mut out = Vec::new();
                for label in KNOWN_LABELS {
                    let mut sql_text = String::from(
                        "SELECT COUNT(*) AS ratings, COALESCE(AVG(score),0) AS avg_score
                         FROM eval_rating WHERE labels_json LIKE ?",
                    );
                    if !f.source.is_empty() {
                        sql_text.push_str(" AND source = ?");
                    }
                    if f.since.is_some() {
                        sql_text.push_str(" AND ts >= ?");
                    }
                    let mut q = sqlx::query(AssertSqlSafe(sql_text)).bind(format!("%\"{label}\"%"));
                    if !f.source.is_empty() {
                        q = q.bind(&f.source);
                    }
                    if let Some(s) = f.since {
                        q = q.bind(clock::to_rfc3339(s));
                    }
                    let row = q.fetch_one(&self.pool).await.map_err(sql)?;
                    let ratings: i64 = row.try_get("ratings").map_err(sql)?;
                    if ratings == 0 {
                        continue; // 걸리지 않은 라벨은 표시하지 않습니다.
                    }
                    out.push(Stat {
                        key: (*label).to_string(),
                        ratings,
                        avg_score: row.try_get("avg_score").map_err(sql)?,
                        ..Default::default()
                    });
                }
                out.sort_by_key(|s| std::cmp::Reverse(s.ratings));
                Ok(out)
            },
        }
    }

    async fn record_run(&self, r: &Run) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO eval_run (run_id, suite, started_at, git_sha, model)
             VALUES (?,?,?,?,?)",
        )
        .bind(&r.run_id)
        .bind(&r.suite)
        .bind(clock::to_rfc3339(r.started_at.unwrap_or_else(Utc::now)))
        .bind(&r.git_sha)
        .bind(&r.model)
        .execute(&self.pool)
        .await
        .map_err(sql)?;
        Ok(())
    }

    async fn add_run_result(&self, r: &RunResult) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO eval_run_result
                (run_id, case_id, pass, score, actual_reply, trail_json, error)
             VALUES (?,?,?,?,?,?,?)",
        )
        .bind(&r.run_id)
        .bind(&r.case_id)
        .bind(r.pass as i64)
        .bind(r.score)
        .bind(&r.actual_reply)
        .bind(serde_json::to_string(&r.trail).unwrap_or_else(|_| "[]".into()))
        .bind(&r.error)
        .execute(&self.pool)
        .await
        .map_err(sql)?;
        Ok(())
    }

    async fn finish_run(&self, run_id: &str, finished_at: DateTime<Utc>, pass: i64, fail: i64) -> Result<(), Error> {
        // 실행 헤더만 예외적으로 UPDATE 합니다 — 결과 row 는 여전히 append-only 입니다.
        let res = sqlx::query("UPDATE eval_run SET finished_at=?, pass_count=?, fail_count=? WHERE run_id=?")
            .bind(clock::to_rfc3339(finished_at))
            .bind(pass)
            .bind(fail)
            .bind(run_id)
            .execute(&self.pool)
            .await
            .map_err(sql)?;
        if res.rows_affected() == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    async fn list_runs(&self, limit: i64) -> Result<Vec<Run>, Error> {
        let rows = sqlx::query(
            "SELECT run_id, suite, started_at, finished_at, git_sha, model, pass_count, fail_count
             FROM eval_run ORDER BY started_at DESC LIMIT ?",
        )
        .bind(if limit <= 0 { 50 } else { limit })
        .fetch_all(&self.pool)
        .await
        .map_err(sql)?;

        rows.iter()
            .map(|r| {
                let started: String = r.try_get("started_at").map_err(sql)?;
                let finished: String = r.try_get("finished_at").map_err(sql)?;
                Ok(Run {
                    run_id: r.try_get("run_id").map_err(sql)?,
                    suite: r.try_get("suite").map_err(sql)?,
                    started_at: clock::parse_rfc3339(&started),
                    finished_at: clock::parse_rfc3339(&finished),
                    git_sha: r.try_get("git_sha").map_err(sql)?,
                    model: r.try_get("model").map_err(sql)?,
                    pass_count: r.try_get("pass_count").map_err(sql)?,
                    fail_count: r.try_get("fail_count").map_err(sql)?,
                })
            })
            .collect()
    }

    async fn get_run_results(&self, run_id: &str) -> Result<Vec<RunResult>, Error> {
        let rows = sqlx::query(
            "SELECT id, run_id, case_id, pass, score, actual_reply, trail_json, error
             FROM eval_run_result WHERE run_id = ? ORDER BY id",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(sql)?;

        rows.iter()
            .map(|r| {
                let trail: String = r.try_get("trail_json").map_err(sql)?;
                Ok(RunResult {
                    id: r.try_get("id").map_err(sql)?,
                    run_id: r.try_get("run_id").map_err(sql)?,
                    case_id: r.try_get("case_id").map_err(sql)?,
                    pass: r.try_get::<i64, _>("pass").map_err(sql)? != 0,
                    score: r.try_get("score").map_err(sql)?,
                    actual_reply: r.try_get("actual_reply").map_err(sql)?,
                    trail: serde_json::from_str(&trail).unwrap_or_default(),
                    error: r.try_get("error").map_err(sql)?,
                })
            })
            .collect()
    }
}

fn sql(e: impl std::fmt::Display) -> Error { Error::Other(anyhow!("eval: {e}")) }

fn is_unique_violation(e: &sqlx::Error) -> bool { matches!(e, sqlx::Error::Database(db) if db.is_unique_violation()) }

fn turn_from_row(r: &sqlx::sqlite::SqliteRow) -> Result<Turn, Error> {
    let ts: String = r.try_get("ts").map_err(sql)?;
    let trail: String = r.try_get("tool_trail_json").map_err(sql)?;
    Ok(Turn {
        turn_id: r.try_get("turn_id").map_err(sql)?,
        timestamp: clock::parse_rfc3339(&ts),
        session_id: r.try_get("session_id").map_err(sql)?,
        actor: r.try_get("actor").map_err(sql)?,
        agent_id: r.try_get("agent_id").map_err(sql)?,
        channel: r.try_get("channel").map_err(sql)?,
        model: r.try_get("model").map_err(sql)?,
        prompt: r.try_get("prompt").map_err(sql)?,
        reply: r.try_get("reply").map_err(sql)?,
        tool_trail: serde_json::from_str::<Vec<ToolStep>>(&trail).unwrap_or_default(),
        outcome: r.try_get("outcome").map_err(sql)?,
        input_tokens: r.try_get("input_tokens").map_err(sql)?,
        output_tokens: r.try_get("output_tokens").map_err(sql)?,
        cost_micros: r.try_get("cost_micros").map_err(sql)?,
        content_hash: r.try_get("content_hash").map_err(sql)?,
    })
}

fn rating_from_row(r: &sqlx::sqlite::SqliteRow) -> Result<Rating, Error> {
    let ts: String = r.try_get("ts").map_err(sql)?;
    let labels: String = r.try_get("labels_json").map_err(sql)?;
    let dims: String = r.try_get("dims_json").map_err(sql)?;
    Ok(Rating {
        id: r.try_get("id").map_err(sql)?,
        turn_id: r.try_get("turn_id").map_err(sql)?,
        timestamp: clock::parse_rfc3339(&ts),
        source: r.try_get("source").map_err(sql)?,
        rater_id: r.try_get("rater_id").map_err(sql)?,
        score: r.try_get("score").map_err(sql)?,
        scale: r.try_get("scale").map_err(sql)?,
        labels: serde_json::from_str(&labels).unwrap_or_default(),
        note: r.try_get("note").map_err(sql)?,
        rubric_id: r.try_get("rubric_id").map_err(sql)?,
        rubric_version: r.try_get("rubric_version").map_err(sql)?,
        dims: serde_json::from_str::<Value>(&dims)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default(),
    })
}

fn stat_from_row(r: &sqlx::sqlite::SqliteRow) -> Stat {
    let key: Option<String> = r.try_get("k").ok().flatten();
    Stat {
        key: key.filter(|k| !k.is_empty()).unwrap_or_else(|| "(none)".into()),
        turns: r.try_get("turns").unwrap_or(0),
        ratings: r.try_get("ratings").unwrap_or(0),
        avg_score: r.try_get("avg_score").unwrap_or(0.0),
        thumbs_up: r.try_get("up").unwrap_or(0),
        thumbs_down: r.try_get("down").unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Scale,
                      Source};

    async fn store() -> SqliteEvalStore { SqliteEvalStore::open_in_memory().await.unwrap() }

    fn turn() -> Turn {
        Turn {
            actor: "emp-sales-01".into(),
            agent_id: "agent-1".into(),
            channel: "chat".into(),
            prompt: "INV-1 결제됐어?".into(),
            reply: "결제 완료되었습니다.".into(),
            outcome: "completed".into(),
            tool_trail: vec![ToolStep {
                name: "get_invoice_status".into(),
                decision: "allowed".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn records_and_reads_a_turn() {
        let s = store().await;
        let t = s.record_turn(&turn()).await.unwrap();
        assert!(t.turn_id.starts_with("turn_"));
        assert!(!t.content_hash.is_empty());

        let got = s.get_turn(&t.turn_id).await.unwrap();
        assert_eq!(got.prompt, "INV-1 결제됐어?");
        assert_eq!(got.tool_trail.len(), 1);
    }

    #[tokio::test]
    async fn turn_snapshots_are_immutable() {
        let s = store().await;
        let mut t = turn();
        t.turn_id = "turn_fixed".into();
        s.record_turn(&t).await.unwrap();
        // 같은 ID 로 다시 쓰면 덮어쓰기가 아니라 오류입니다.
        assert!(matches!(s.record_turn(&t).await.unwrap_err(), Error::Invalid(_)));
    }

    #[tokio::test]
    async fn ratings_are_append_only_and_coexist() {
        let s = store().await;
        let t = s.record_turn(&turn()).await.unwrap();

        let user = Rating {
            turn_id: t.turn_id.clone(),
            source: Source::HumanUser.as_str().into(),
            scale: Scale::Thumbs.as_str().into(),
            rater_id: "emp-sales-01".into(),
            score: 0.0,
            labels: vec!["wrong_fact".into()],
            ..Default::default()
        };
        s.rate(&user).await.unwrap();

        // 관리자 재라벨은 사용자 점수를 **덮어쓰지 않습니다.**
        let reviewer = Rating {
            turn_id: t.turn_id.clone(),
            source: Source::HumanReviewer.as_str().into(),
            scale: Scale::Thumbs.as_str().into(),
            rater_id: "manager-01".into(),
            score: 1.0,
            ..Default::default()
        };
        s.rate(&reviewer).await.unwrap();

        let all = s
            .query_ratings(&RatingFilter {
                turn_id: t.turn_id.clone(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(all.len(), 2, "재라벨이 기존 점수를 덮어썼습니다");
    }

    #[tokio::test]
    async fn rating_a_missing_turn_is_not_found() {
        let s = store().await;
        let r = Rating {
            turn_id: "turn_nope".into(),
            source: "human_user".into(),
            scale: "thumbs".into(),
            rater_id: "u".into(),
            score: 1.0,
            ..Default::default()
        };
        assert!(matches!(s.rate(&r).await.unwrap_err(), Error::NotFound));
    }

    #[tokio::test]
    async fn unrated_filter_finds_turns_without_ratings() {
        let s = store().await;
        let a = s.record_turn(&turn()).await.unwrap();
        let b = s.record_turn(&turn()).await.unwrap();

        s.rate(&Rating {
            turn_id: a.turn_id,
            source: "human_user".into(),
            scale: "thumbs".into(),
            rater_id: "u".into(),
            score: 1.0,
            ..Default::default()
        })
        .await
        .unwrap();

        let unrated = s
            .query_turns(&TurnFilter {
                unrated_only: true,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(unrated.len(), 1);
        assert_eq!(unrated[0].turn_id, b.turn_id);
    }

    #[tokio::test]
    async fn agent_stats_keep_unrated_turns_in_the_denominator() {
        // source 필터를 WHERE 에 두면 미평가 턴이 통계에서 통째로 사라집니다.
        let s = store().await;
        s.record_turn(&turn()).await.unwrap();
        let b = s.record_turn(&turn()).await.unwrap();
        s.rate(&Rating {
            turn_id: b.turn_id,
            source: "human_user".into(),
            scale: "thumbs".into(),
            rater_id: "u".into(),
            score: 1.0,
            ..Default::default()
        })
        .await
        .unwrap();

        let st = s
            .stats(
                GroupBy::Agent,
                &StatFilter {
                    source: "human_user".into(),
                    scale: "thumbs".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].turns, 2, "미평가 턴이 분모에서 사라졌습니다");
        assert_eq!(st[0].ratings, 1);
        assert_eq!(st[0].thumbs_up, 1);
    }

    #[tokio::test]
    async fn golden_run_lifecycle() {
        let s = store().await;
        let run = Run {
            run_id: "run_1".into(),
            suite: "erp-read".into(),
            started_at: Some(Utc::now()),
            model: "scripted-gateway".into(),
            ..Default::default()
        };
        s.record_run(&run).await.unwrap();
        s.add_run_result(&RunResult {
            run_id: "run_1".into(),
            case_id: "c1".into(),
            pass: true,
            score: 1.0,
            ..Default::default()
        })
        .await
        .unwrap();
        s.finish_run("run_1", Utc::now(), 1, 0).await.unwrap();

        let runs = s.list_runs(10).await.unwrap();
        assert_eq!(runs[0].pass_count, 1);
        let results = s.get_run_results("run_1").await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].pass);
    }
}
