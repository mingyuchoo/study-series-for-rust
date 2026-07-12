//! 관계형 DB 전송 (`-erp-db`).
//!
//! 레거시가 API 를 내주지 않고 DB 만 여는 경우가 흔합니다. 어댑터는 여전히
//! 프로토콜을 모릅니다 — "송장 하나를 가져온다"는 의도가 여기서 `SELECT` 로
//! 바뀔 뿐입니다.
//!
//! **LLM 이 임의 SQL 을 쓸 수 있는 통로가 아닙니다.** 실행되는 쿼리는 이 파일에
//! 고정된 것뿐이고, 인자는 전부 bind 파라미터로 들어갑니다.

use super::{Operation,
            Transport,
            not_found};
use ai_bridge::transient;
use anyhow::{Result,
             anyhow};
use serde_json::{Map,
                 Value};
use sqlx::{AssertSqlSafe,
           Column,
           Row,
           TypeInfo};

/// 지원하는 백엔드.
#[derive(Debug, Clone)]
enum Pool {
    Sqlite(sqlx::SqlitePool),
    Postgres(sqlx::PgPool),
}

#[derive(Debug, Clone)]
pub struct DbTransport {
    pool: Pool,
    dsn: String,
}

impl DbTransport {
    /// `postgres://…` 이면 PostgreSQL, 그 밖에는 SQLite 파일 경로로 봅니다.
    pub async fn open(dsn: &str) -> Result<Self> {
        let pool = if dsn.starts_with("postgres://") || dsn.starts_with("postgresql://") {
            Pool::Postgres(sqlx::postgres::PgPoolOptions::new().max_connections(5).connect(dsn).await?)
        } else {
            let opts = sqlx::sqlite::SqliteConnectOptions::new().filename(dsn).create_if_missing(true);
            Pool::Sqlite(sqlx::sqlite::SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?)
        };
        let t = Self {
            pool,
            dsn: dsn.to_string(),
        };
        t.migrate().await?;
        Ok(t)
    }

    /// 데모용 ERP 스키마. 실제 레거시라면 이미 존재하는 테이블에 붙습니다.
    async fn migrate(&self) -> Result<()> {
        let ddl = "CREATE TABLE IF NOT EXISTS invoice (
                     invoice_id  TEXT PRIMARY KEY,
                     customer_id TEXT NOT NULL,
                     status      TEXT NOT NULL,
                     amount      BIGINT NOT NULL,
                     issued_at   TEXT NOT NULL,
                     paid_at     TEXT
                   )";
        match &self.pool {
            | Pool::Sqlite(p) => {
                sqlx::query(AssertSqlSafe(ddl.to_string())).execute(p).await?;
            },
            | Pool::Postgres(p) => {
                sqlx::query(AssertSqlSafe(ddl.to_string())).execute(p).await?;
            },
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Transport for DbTransport {
    async fn call(&self, op: &Operation) -> Result<Value> {
        // 실행 가능한 쿼리는 **여기 적힌 것뿐입니다.**
        match op.name.as_str() {
            | "get_invoice" => {
                let id = op.path.last().ok_or_else(|| anyhow!("get_invoice: invoice id is required"))?;
                let sql = "SELECT invoice_id, customer_id, status, amount, issued_at, paid_at
                           FROM invoice WHERE invoice_id = ?";
                let rows = self.fetch(sql, std::slice::from_ref(id)).await?;
                rows.into_iter().next().ok_or_else(|| not_found(format!("송장 {id}")))
            },
            | "list_customer_invoices" => {
                let cid = op
                    .params
                    .get("customer_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("list_customer_invoices: customer_id is required"))?;
                let sql = "SELECT invoice_id, customer_id, status, amount, issued_at, paid_at
                           FROM invoice WHERE customer_id = ? ORDER BY issued_at DESC";
                let rows = self.fetch(sql, &[cid.to_string()]).await?;
                Ok(serde_json::json!({ "invoices": rows }))
            },
            | other => Err(anyhow!("db transport: unsupported operation {other:?}")),
        }
    }

    async fn health(&self) -> Result<()> {
        let r = match &self.pool {
            | Pool::Sqlite(p) => sqlx::query("SELECT 1").execute(p).await.map(|_| ()),
            | Pool::Postgres(p) => sqlx::query("SELECT 1").execute(p).await.map(|_| ()),
        };
        // DB 가 안 열리는 것은 "지금 안 되는 것" 입니다.
        r.map_err(|e| transient::temporary(anyhow!("db health: {e}")))
    }

    fn describe(&self) -> String { format!("db {}", self.dsn) }
}

impl DbTransport {
    async fn fetch(&self, sql: &str, binds: &[String]) -> Result<Vec<Value>> {
        match &self.pool {
            | Pool::Sqlite(p) => {
                let mut q = sqlx::query(AssertSqlSafe(sql.to_string()));
                for b in binds {
                    q = q.bind(b);
                }
                let rows = q.fetch_all(p).await.map_err(|e| transient::temporary(anyhow!("db query: {e}")))?;
                Ok(rows.iter().map(sqlite_row_to_json).collect())
            },
            | Pool::Postgres(p) => {
                // PostgreSQL 은 `$1` 형식을 씁니다.
                let mut n = 0;
                let pg_sql: String = sql
                    .chars()
                    .map(|c| {
                        if c == '?' {
                            n += 1;
                            format!("${n}")
                        } else {
                            c.to_string()
                        }
                    })
                    .collect();
                let mut q = sqlx::query(AssertSqlSafe(pg_sql));
                for b in binds {
                    q = q.bind(b);
                }
                let rows = q.fetch_all(p).await.map_err(|e| transient::temporary(anyhow!("db query: {e}")))?;
                Ok(rows.iter().map(pg_row_to_json).collect())
            },
        }
    }
}

fn sqlite_row_to_json(r: &sqlx::sqlite::SqliteRow) -> Value {
    let mut out = Map::new();
    for c in r.columns() {
        let name = c.name();
        let v: Value = match c.type_info().name() {
            | "INTEGER" => r.try_get::<Option<i64>, _>(name).ok().flatten().map(Value::from).unwrap_or(Value::Null),
            | "REAL" => r.try_get::<Option<f64>, _>(name).ok().flatten().map(Value::from).unwrap_or(Value::Null),
            | _ => r.try_get::<Option<String>, _>(name).ok().flatten().map(Value::from).unwrap_or(Value::Null),
        };
        out.insert(name.to_string(), v);
    }
    Value::Object(out)
}

fn pg_row_to_json(r: &sqlx::postgres::PgRow) -> Value {
    let mut out = Map::new();
    for c in r.columns() {
        let name = c.name();
        let v: Value = match c.type_info().name() {
            | "INT8" | "INT4" | "INT2" => r.try_get::<Option<i64>, _>(name).ok().flatten().map(Value::from).unwrap_or(Value::Null),
            | "FLOAT8" | "FLOAT4" => r.try_get::<Option<f64>, _>(name).ok().flatten().map(Value::from).unwrap_or(Value::Null),
            | _ => r.try_get::<Option<String>, _>(name).ok().flatten().map(Value::from).unwrap_or(Value::Null),
        };
        out.insert(name.to_string(), v);
    }
    Value::Object(out)
}
