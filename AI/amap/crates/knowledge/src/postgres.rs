//! PostgreSQL system-of-record via SQLx. Documents are stored as JSONB with typed key columns,
//! so the schema (see `migrations/`) stays stable while domain types evolve.
use crate::*;
use serde::de::DeserializeOwned;
use sqlx::{postgres::PgPoolOptions, AssertSqlSafe, PgPool, Row};

pub struct PgKnowledgeStore {
    pool: PgPool,
}

impl PgKnowledgeStore {
    pub async fn connect(url: &str) -> KResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .connect(url)
            .await
            .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Apply the SQL migrations embedded from `migrations/`.
    pub async fn migrate(&self) -> KResult<()> {
        sqlx::migrate!("../../migrations")
            .run(&self.pool)
            .await
            .map_err(|e| KnowledgeError::Storage(e.to_string()))
    }

    async fn upsert<T: Serialize>(
        &self,
        table: &str,
        id: &str,
        function_id: Option<&str>,
        doc: &T,
    ) -> KResult<()> {
        let sql = format!(
            "INSERT INTO {table} (id, function_id, doc, updated_at) VALUES ($1, $2, $3, now()) \
             ON CONFLICT (id) DO UPDATE SET function_id = EXCLUDED.function_id, doc = EXCLUDED.doc, updated_at = now()"
        );
        sqlx::query(AssertSqlSafe(sql))
            .bind(id)
            .bind(function_id)
            .bind(serde_json::to_value(doc)?)
            .execute(&self.pool)
            .await
            .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn list<T: DeserializeOwned>(
        &self,
        table: &str,
        function_id: Option<&str>,
    ) -> KResult<Vec<T>> {
        let rows = match function_id {
            Some(f) => {
                sqlx::query(AssertSqlSafe(format!(
                    "SELECT doc FROM {table} WHERE function_id = $1 ORDER BY id"
                )))
                .bind(f)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query(AssertSqlSafe(format!(
                    "SELECT doc FROM {table} ORDER BY id"
                )))
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        rows.into_iter()
            .map(|r| {
                serde_json::from_value(r.get::<serde_json::Value, _>("doc")).map_err(Into::into)
            })
            .collect()
    }

    async fn get<T: DeserializeOwned>(&self, table: &str, id: &str) -> KResult<Option<T>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT doc FROM {table} WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        row.map(|r| {
            serde_json::from_value(r.get::<serde_json::Value, _>("doc")).map_err(Into::into)
        })
        .transpose()
    }
}

#[async_trait]
impl KnowledgeStore for PgKnowledgeStore {
    async fn upsert_function(&self, f: BusinessFunction) -> KResult<()> {
        self.upsert("business_functions", f.id.as_str(), Some(f.id.as_str()), &f)
            .await
    }
    async fn get_function(&self, id: &FunctionId) -> KResult<Option<BusinessFunction>> {
        self.get("business_functions", id.as_str()).await
    }
    async fn list_functions(&self) -> KResult<Vec<BusinessFunction>> {
        self.list("business_functions", None).await
    }
    async fn upsert_requirement(&self, r: Requirement) -> KResult<()> {
        self.upsert(
            "requirements",
            r.id.as_str(),
            Some(r.function_id.as_str()),
            &r,
        )
        .await
    }
    async fn requirements_for(&self, f: &FunctionId) -> KResult<Vec<Requirement>> {
        self.list("requirements", Some(f.as_str())).await
    }
    async fn upsert_rule(&self, r: BusinessRule) -> KResult<()> {
        self.upsert(
            "business_rules",
            r.id.as_str(),
            Some(r.function_id.as_str()),
            &r,
        )
        .await
    }
    async fn rules_for(&self, f: &FunctionId) -> KResult<Vec<BusinessRule>> {
        self.list("business_rules", Some(f.as_str())).await
    }
    async fn upsert_source_unit(&self, e: CodeEntity) -> KResult<()> {
        let fid = e.function_id.as_ref().map(|f| f.as_str().to_string());
        self.upsert("source_units", e.id.as_str(), fid.as_deref(), &e)
            .await
    }
    async fn source_units_for(&self, f: &FunctionId) -> KResult<Vec<CodeEntity>> {
        self.list("source_units", Some(f.as_str())).await
    }
    async fn list_source_units(&self) -> KResult<Vec<CodeEntity>> {
        self.list("source_units", None).await
    }
    async fn upsert_db_entity(&self, e: DbEntity) -> KResult<()> {
        self.upsert("db_entities", e.id.as_str(), None, &e).await
    }
    async fn list_db_entities(&self) -> KResult<Vec<DbEntity>> {
        self.list("db_entities", None).await
    }
    async fn upsert_interface(&self, i: InterfaceSpec) -> KResult<()> {
        self.upsert("interfaces", i.id.as_str(), None, &i).await
    }
    async fn list_interfaces(&self) -> KResult<Vec<InterfaceSpec>> {
        self.list("interfaces", None).await
    }
    async fn upsert_behavior(&self, b: BehaviorRecord) -> KResult<()> {
        self.upsert("behaviors", b.id.as_str(), Some(b.function_id.as_str()), &b)
            .await
    }
    async fn behaviors_for(&self, f: &FunctionId) -> KResult<Vec<BehaviorRecord>> {
        self.list("behaviors", Some(f.as_str())).await
    }
    async fn upsert_scenario(&self, s: TestScenario) -> KResult<()> {
        self.upsert(
            "test_cases",
            s.id.as_str(),
            Some(s.function_id.as_str()),
            &s,
        )
        .await
    }
    async fn scenarios_for(&self, f: &FunctionId) -> KResult<Vec<TestScenario>> {
        self.list("test_cases", Some(f.as_str())).await
    }
    async fn upsert_decision(&self, d: ArchitectureDecision) -> KResult<()> {
        self.upsert(
            "architecture_decisions",
            d.id.as_str(),
            Some(d.function_id.as_str()),
            &d,
        )
        .await
    }
    async fn decisions_for(&self, f: &FunctionId) -> KResult<Vec<ArchitectureDecision>> {
        self.list("architecture_decisions", Some(f.as_str())).await
    }
    async fn add_relationship(&self, r: Relationship) -> KResult<()> {
        sqlx::query("INSERT INTO relationships (from_id, to_id, kind) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(&r.from)
            .bind(&r.to)
            .bind(serde_json::to_value(r.kind)?.as_str().unwrap_or("unknown").to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        Ok(())
    }
    async fn relationships(&self) -> KResult<Vec<Relationship>> {
        let rows = sqlx::query("SELECT from_id, to_id, kind FROM relationships")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        rows.into_iter()
            .map(|r| {
                let kind: String = r.get("kind");
                Ok(Relationship {
                    from: r.get("from_id"),
                    to: r.get("to_id"),
                    kind: serde_json::from_value(serde_json::Value::String(kind))?,
                })
            })
            .collect()
    }
    async fn record_evidence(&self, e: EvidenceRecord) -> KResult<()> {
        sqlx::query(
            "INSERT INTO evidence (evidence_id, content_hash, payload_uri, run_id, function_id, kind, scenario_id, priority, passed, explained, producer, created_at, doc) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) ON CONFLICT (evidence_id) DO NOTHING",
        )
        .bind(&e.evidence_id)
        .bind(&e.content_hash)
        .bind(&e.payload_uri)
        .bind(e.run_id.as_str())
        .bind(e.function_id.as_str())
        .bind(serde_json::to_value(e.kind)?.as_str().unwrap_or("").to_string())
        .bind(&e.scenario_id)
        .bind(format!("{:?}", e.priority))
        .bind(e.passed)
        .bind(e.explained)
        .bind(&e.producer)
        .bind(e.created_at)
        .bind(serde_json::to_value(&e)?)
        .execute(&self.pool)
        .await
        .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        Ok(())
    }
    async fn evidence_for(&self, f: &FunctionId) -> KResult<Vec<EvidenceRecord>> {
        let rows =
            sqlx::query("SELECT doc FROM evidence WHERE function_id = $1 ORDER BY created_at")
                .bind(f.as_str())
                .fetch_all(&self.pool)
                .await
                .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        rows.into_iter()
            .map(|r| {
                serde_json::from_value(r.get::<serde_json::Value, _>("doc")).map_err(Into::into)
            })
            .collect()
    }
    async fn queue_review(&self, r: ReviewRequest) -> KResult<()> {
        let result = sqlx::query(
            "INSERT INTO review_requests (id, function_id, doc, updated_at) VALUES ($1, $2, $3, now()) ON CONFLICT (id) DO NOTHING",
        )
        .bind(&r.id)
        .bind(r.function_id.as_str())
        .bind(serde_json::to_value(&r)?)
        .execute(&self.pool)
        .await
        .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        if result.rows_affected() == 0 {
            return Err(KnowledgeError::Storage(format!(
                "review {} already exists",
                r.id
            )));
        }
        Ok(())
    }
    async fn list_reviews(&self) -> KResult<Vec<ReviewRequest>> {
        self.list("review_requests", None).await
    }
    async fn decide_review(&self, id: &str, status: ReviewStatus, by: &str) -> KResult<()> {
        let mut r: ReviewRequest = self
            .get("review_requests", id)
            .await?
            .ok_or_else(|| KnowledgeError::NotFound(id.into()))?;
        if r.status != ReviewStatus::Pending {
            return Err(KnowledgeError::Storage(format!(
                "review {id} has already been decided"
            )));
        }
        r.status = status;
        r.decided_by = Some(by.to_string());
        r.decided_at = Some(chrono::Utc::now());
        let result = sqlx::query(
            "UPDATE review_requests SET doc = $2, updated_at = now() WHERE id = $1 AND doc->>'status' = 'pending'",
        )
        .bind(id)
        .bind(serde_json::to_value(&r)?)
        .execute(&self.pool)
        .await
        .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        if result.rows_affected() == 0 {
            return Err(KnowledgeError::Storage(format!(
                "review {id} has already been decided"
            )));
        }
        Ok(())
    }
    async fn upsert_workflow_run(&self, run: WorkflowRun) -> KResult<()> {
        self.upsert(
            "workflow_runs",
            &run.id,
            Some(run.function_id.as_str()),
            &run,
        )
        .await
    }
    async fn get_workflow_run(&self, id: &str) -> KResult<Option<WorkflowRun>> {
        self.get("workflow_runs", id).await
    }
    async fn list_workflow_runs(&self) -> KResult<Vec<WorkflowRun>> {
        self.list("workflow_runs", None).await
    }
    async fn snapshot(&self) -> KResult<KnowledgeSnapshot> {
        let rows = sqlx::query("SELECT doc FROM evidence ORDER BY created_at")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| KnowledgeError::Storage(e.to_string()))?;
        let evidence = rows
            .into_iter()
            .map(|r| {
                serde_json::from_value(r.get::<serde_json::Value, _>("doc"))
                    .map_err(KnowledgeError::from)
            })
            .collect::<KResult<Vec<_>>>()?;
        Ok(KnowledgeSnapshot {
            functions: self.list("business_functions", None).await?,
            requirements: self.list("requirements", None).await?,
            rules: self.list("business_rules", None).await?,
            source_units: self.list("source_units", None).await?,
            db_entities: self.list("db_entities", None).await?,
            interfaces: self.list("interfaces", None).await?,
            behaviors: self.list("behaviors", None).await?,
            scenarios: self.list("test_cases", None).await?,
            decisions: self.list("architecture_decisions", None).await?,
            relationships: self.relationships().await?,
            evidence,
        })
    }
}
