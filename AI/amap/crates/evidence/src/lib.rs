//! Evidence Lake / Evidence Ledger (stack §5, §6).
//!
//! Verification produces enormous volumes of records. They are appended as Parquet files to a
//! local directory or an S3/MinIO bucket, and queried in-process with Apache DataFusion.
//! The [`FunctionCertificate`] is computed from the ledger with SQL — never asserted by an agent.

use amap_domain::*;
use datafusion::arrow::array::{ArrayRef, BooleanArray, StringArray};
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::arrow::json::ArrayWriter;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::parquet::arrow::ArrowWriter;
use datafusion::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum EvidenceError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("query error: {0}")]
    Query(String),
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<datafusion::error::DataFusionError> for EvidenceError {
    fn from(e: datafusion::error::DataFusionError) -> Self {
        EvidenceError::Query(e.to_string())
    }
}

/// Where the lake lives.
#[derive(Clone, Debug)]
pub enum LakeLocation {
    Local(PathBuf),
    /// `s3://bucket/prefix` — credentials from the environment (AWS_* / MinIO endpoint via AWS_ENDPOINT_URL).
    S3 {
        url: Url,
    },
}

pub struct EvidenceLake {
    location: LakeLocation,
    ctx: SessionContext,
}

fn schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("evidence_id", DataType::Utf8, false),
        Field::new("content_hash", DataType::Utf8, false),
        Field::new("run_id", DataType::Utf8, false),
        Field::new("function_id", DataType::Utf8, false),
        Field::new("kind", DataType::Utf8, false),
        Field::new("scenario_id", DataType::Utf8, false),
        Field::new("rule_ids", DataType::Utf8, false),
        Field::new("priority", DataType::Utf8, false),
        Field::new("passed", DataType::Boolean, false),
        Field::new("explained", DataType::Boolean, false),
        Field::new("producer", DataType::Utf8, false),
        Field::new("created_at", DataType::Utf8, false),
        Field::new("payload_uri", DataType::Utf8, true),
        Field::new("details", DataType::Utf8, false),
    ]))
}

fn kind_str(k: VerificationKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

fn to_batch(records: &[EvidenceRecord]) -> RecordBatch {
    let s = |f: &dyn Fn(&EvidenceRecord) -> String| -> ArrayRef {
        Arc::new(StringArray::from(records.iter().map(f).collect::<Vec<_>>()))
    };
    let cols: Vec<ArrayRef> = vec![
        s(&|r| r.evidence_id.clone()),
        s(&|r| r.content_hash.clone()),
        s(&|r| r.run_id.0.clone()),
        s(&|r| r.function_id.0.clone()),
        s(&|r| kind_str(r.kind)),
        s(&|r| r.scenario_id.clone()),
        s(&|r| {
            r.rule_ids
                .iter()
                .map(|x| x.0.clone())
                .collect::<Vec<_>>()
                .join(",")
        }),
        s(&|r| format!("{:?}", r.priority)),
        Arc::new(BooleanArray::from(
            records.iter().map(|r| r.passed).collect::<Vec<_>>(),
        )),
        Arc::new(BooleanArray::from(
            records.iter().map(|r| r.explained).collect::<Vec<_>>(),
        )),
        s(&|r| r.producer.clone()),
        s(&|r| r.created_at.to_rfc3339()),
        Arc::new(StringArray::from(
            records
                .iter()
                .map(|r| r.payload_uri.clone())
                .collect::<Vec<Option<String>>>(),
        )),
        s(&|r| r.details.to_string()),
    ];
    RecordBatch::try_new(schema(), cols).expect("schema matches columns")
}

impl EvidenceLake {
    pub async fn open_local(root: impl AsRef<Path>) -> Result<Self, EvidenceError> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(root.join("evidence"))?;
        let ctx = SessionContext::new();
        Ok(Self {
            location: LakeLocation::Local(root),
            ctx,
        })
    }

    /// Open an S3 / MinIO backed lake (`s3://bucket/prefix`).
    pub async fn open_s3(url: &str) -> Result<Self, EvidenceError> {
        let url = Url::parse(url).map_err(|e| EvidenceError::Storage(e.to_string()))?;
        let bucket = url
            .host_str()
            .ok_or_else(|| EvidenceError::Storage("s3 url needs bucket".into()))?;
        let store = object_store::aws::AmazonS3Builder::from_env()
            .with_bucket_name(bucket)
            .build()
            .map_err(|e| EvidenceError::Storage(e.to_string()))?;
        let ctx = SessionContext::new();
        let base = Url::parse(&format!("s3://{bucket}/")).unwrap();
        ctx.register_object_store(&base, Arc::new(store));
        Ok(Self {
            location: LakeLocation::S3 { url },
            ctx,
        })
    }

    fn evidence_uri(&self) -> String {
        match &self.location {
            LakeLocation::Local(root) => format!("{}/", root.join("evidence").display()),
            LakeLocation::S3 { url } => format!("{}/evidence/", url.as_str().trim_end_matches('/')),
        }
    }

    /// Append a batch of records as one Parquet file. Returns the object URI.
    pub async fn append(&self, records: &[EvidenceRecord]) -> Result<String, EvidenceError> {
        if records.is_empty() {
            return Ok(String::new());
        }
        let name = format!("{}-{}.parquet", records[0].run_id.0, uuid::Uuid::new_v4());
        let uri = match &self.location {
            LakeLocation::Local(root) => root.join("evidence").join(&name).display().to_string(),
            LakeLocation::S3 { url } => {
                let bucket = url.host_str().unwrap_or_default();
                let prefix = url.path().trim_matches('/');
                let key = if prefix.is_empty() {
                    format!("evidence/{name}")
                } else {
                    format!("{prefix}/evidence/{name}")
                };
                format!("s3://{bucket}/{key}")
            }
        };
        let stored: Vec<EvidenceRecord> = records
            .iter()
            .cloned()
            .map(|mut record| {
                record.payload_uri = Some(uri.clone());
                record
            })
            .collect();
        let batch = to_batch(&stored);
        let mut buf = Vec::new();
        {
            let mut w = ArrowWriter::try_new(&mut buf, schema(), None)
                .map_err(|e| EvidenceError::Storage(e.to_string()))?;
            w.write(&batch)
                .map_err(|e| EvidenceError::Storage(e.to_string()))?;
            w.close()
                .map_err(|e| EvidenceError::Storage(e.to_string()))?;
        }
        match &self.location {
            LakeLocation::Local(root) => {
                let path = root.join("evidence").join(&name);
                tokio::fs::write(&path, buf).await?;
                Ok(uri)
            }
            LakeLocation::S3 { url } => {
                let bucket = url.host_str().unwrap_or_default();
                let store = object_store::aws::AmazonS3Builder::from_env()
                    .with_bucket_name(bucket)
                    .build()
                    .map_err(|e| EvidenceError::Storage(e.to_string()))?;
                let prefix = url.path().trim_matches('/');
                let key = if prefix.is_empty() {
                    format!("evidence/{name}")
                } else {
                    format!("{prefix}/evidence/{name}")
                };
                use object_store::ObjectStoreExt;
                store
                    .put(&object_store::path::Path::from(key.clone()), buf.into())
                    .await
                    .map_err(|e| EvidenceError::Storage(e.to_string()))?;
                Ok(uri)
            }
        }
    }

    /// Run SQL over the `evidence` table (all Parquet files in the lake).
    pub async fn query(&self, sql: &str) -> Result<Vec<Value>, EvidenceError> {
        let ctx = SessionContext::new();
        if let LakeLocation::S3 { url } = &self.location {
            let bucket = url.host_str().unwrap_or_default();
            let store = object_store::aws::AmazonS3Builder::from_env()
                .with_bucket_name(bucket)
                .build()
                .map_err(|e| EvidenceError::Storage(e.to_string()))?;
            ctx.register_object_store(
                &Url::parse(&format!("s3://{bucket}/")).unwrap(),
                Arc::new(store),
            );
        }
        let _ = &self.ctx;
        let uri = self.evidence_uri();
        let has_files = match &self.location {
            LakeLocation::Local(root) => std::fs::read_dir(root.join("evidence"))?.any(|e| {
                e.map(|e| {
                    e.path()
                        .extension()
                        .map(|x| x == "parquet")
                        .unwrap_or(false)
                })
                .unwrap_or(false)
            }),
            LakeLocation::S3 { .. } => true,
        };
        if !has_files {
            ctx.register_batch("evidence", RecordBatch::new_empty(schema()))?;
        } else {
            ctx.register_parquet(
                "evidence",
                &uri,
                ParquetReadOptions::default().schema(&schema()),
            )
            .await?;
        }
        let df = ctx.sql(sql).await?;
        let batches = df.collect().await?;
        let mut out = Vec::new();
        {
            let mut w = ArrayWriter::new(&mut out);
            w.write_batches(&batches.iter().collect::<Vec<_>>())
                .map_err(|e| EvidenceError::Query(e.to_string()))?;
            w.finish()
                .map_err(|e| EvidenceError::Query(e.to_string()))?;
        }
        if out.is_empty() {
            return Ok(vec![]);
        }
        serde_json::from_slice(&out).map_err(|e| EvidenceError::Query(e.to_string()))
    }

    /// Aggregate pass/fail counts per verification kind for a function.
    pub async fn summary(&self, function_id: &str) -> Result<Vec<KindRow>, EvidenceError> {
        let rows = self
            .query(&format!(
                "SELECT kind, priority, count(*) AS total, sum(CASE WHEN passed THEN 1 ELSE 0 END) AS passed, \
                 sum(CASE WHEN NOT passed AND NOT explained THEN 1 ELSE 0 END) AS unexplained \
                 FROM evidence WHERE function_id = '{}' GROUP BY kind, priority ORDER BY kind, priority",
                function_id.replace('\'', "''")
            ))
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| KindRow {
                kind: r["kind"].as_str().unwrap_or_default().to_string(),
                priority: r["priority"].as_str().unwrap_or_default().to_string(),
                total: r["total"].as_u64().unwrap_or(0),
                passed: r["passed"].as_u64().unwrap_or(0),
                unexplained: r["unexplained"].as_u64().unwrap_or(0),
            })
            .collect())
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KindRow {
    pub kind: String,
    pub priority: String,
    pub total: u64,
    pub passed: u64,
    pub unexplained: u64,
}

/// Build a [`FunctionCertificate`] + [`EquivalenceMetrics`] from ledger rows and knowledge counts.
pub struct CertificateInputs {
    pub function_id: String,
    pub implemented: bool,
    pub requirements_total: u64,
    pub requirements_covered: u64,
    pub rules_total: u64,
    pub rules_covered: u64,
    pub critical_rules_total: u64,
    pub critical_rules_covered: u64,
    pub behaviors_total: u64,
    pub behaviors_covered: u64,
    pub mutation_injected: u64,
    pub mutation_detected: u64,
    pub residual_uncertainty: f64,
}

pub fn build_certificate(
    rows: &[KindRow],
    inputs: CertificateInputs,
) -> (FunctionCertificate, EquivalenceMetrics) {
    let mut cert = FunctionCertificate {
        function_id: inputs.function_id,
        implemented: inputs.implemented,
        requirements_total: inputs.requirements_total,
        requirements_covered: inputs.requirements_covered,
        rules_total: inputs.rules_total,
        rules_covered: inputs.rules_covered,
        critical_rules_total: inputs.critical_rules_total,
        critical_rules_covered: inputs.critical_rules_covered,
        mutation_injected: inputs.mutation_injected,
        mutation_detected: inputs.mutation_detected,
        residual_uncertainty: inputs.residual_uncertainty,
        ..Default::default()
    };
    let mut eq = EquivalenceMetrics {
        behaviors_total: inputs.behaviors_total,
        behaviors_covered: inputs.behaviors_covered,
        ..Default::default()
    };
    for r in rows {
        let target = match r.kind.as_str() {
            "golden_replay" => Some(&mut cert.golden),
            "boundary" => Some(&mut cert.boundary),
            "property" | "unit" => Some(&mut cert.property),
            "adversarial" => Some(&mut cert.adversarial),
            "fault" => Some(&mut cert.fault),
            "concurrency" => Some(&mut cert.concurrency),
            "differential" | "state" | "interface" => Some(&mut cert.production_replay),
            _ => None,
        };
        if let Some(t) = target {
            t.total += r.total;
            t.passed += r.passed;
            t.unexplained_failures += r.unexplained;
        }
        if r.kind != "mutation" {
            cert.unexplained_differences += r.unexplained;
        }
        if matches!(r.kind.as_str(), "golden_replay" | "differential" | "unit") {
            eq.all_total += r.total;
            eq.all_passed += r.passed;
            match r.priority.as_str() {
                "P0" => {
                    eq.p0_total += r.total;
                    eq.p0_passed += r.passed;
                }
                "P1" => {
                    eq.p1_total += r.total;
                    eq.p1_passed += r.passed;
                }
                _ => {}
            }
            if r.unexplained > 0 && r.priority == "P0" {
                cert.p0_defects_open += r.unexplained;
            }
            if r.unexplained > 0 && r.priority == "P1" {
                cert.p1_defects_open += r.unexplained;
            }
        }
    }
    (cert, eq)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn append_and_query_local() {
        let dir = std::env::temp_dir().join(format!("amap-lake-{}", uuid::Uuid::new_v4()));
        let lake = EvidenceLake::open_local(&dir).await.unwrap();
        let rec = |passed: bool| EvidenceRecord {
            evidence_id: format!("EV-{}", uuid::Uuid::new_v4()),
            content_hash: "test-hash".into(),
            run_id: RunId::new("RUN-1"),
            function_id: FunctionId::new("FN-1"),
            kind: VerificationKind::GoldenReplay,
            scenario_id: "TEST-1".into(),
            rule_ids: vec![RuleId::new("BR-1")],
            priority: Priority::P0,
            passed,
            explained: false,
            producer: "replay-worker".into(),
            created_at: Utc::now(),
            payload_uri: None,
            details: serde_json::json!({}),
        };
        lake.append(&[rec(true), rec(true), rec(false)])
            .await
            .unwrap();
        let rows = lake.summary("FN-1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].total, 3);
        assert_eq!(rows[0].passed, 2);
        assert_eq!(rows[0].unexplained, 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
