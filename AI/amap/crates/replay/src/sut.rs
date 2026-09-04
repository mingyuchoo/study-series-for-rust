//! System-under-test adapters: JSON-lines subprocess (batch) and HTTP.
use crate::ReplayError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExecOptions {
    /// Fault to inject (e.g. `db_timeout`, `api_timeout`, `mq_duplicate`, `partial_commit`).
    #[serde(default)]
    pub fault: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Concurrency schedule (ordered steps) for concurrency verification.
    #[serde(default)]
    pub schedule: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayCase {
    pub id: String,
    pub initial_state: Value,
    pub input: Value,
    pub options: ExecOptions,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Execution {
    #[serde(default)]
    pub output: Value,
    #[serde(default)]
    pub state_change: Value,
    #[serde(default)]
    pub events: Vec<Value>,
    #[serde(default)]
    pub external_calls: Vec<Value>,
    #[serde(default)]
    pub duration_ms: u64,
}

#[async_trait]
pub trait SystemUnderTest: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, case: &ReplayCase) -> Result<Execution, ReplayError>;
    /// Batch execution; the default runs cases sequentially.
    async fn execute_batch(&self, cases: &[ReplayCase]) -> Vec<Result<Execution, ReplayError>> {
        let mut out = Vec::with_capacity(cases.len());
        for c in cases {
            out.push(self.execute(c).await);
        }
        out
    }
}

impl Clone for ReplayError {
    fn clone(&self) -> Self {
        ReplayError::Sut(self.to_string())
    }
}

/// Runs a program that reads JSON lines `{id, initial_state, input, options}` on stdin and writes one
/// JSON line `{id?, output, state_change, events, external_calls}` per case on stdout.
#[derive(Clone, Debug)]
pub struct ProcessSystem {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub workdir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub timeout: Duration,
}

impl ProcessSystem {
    pub fn new(name: &str, program: &str, args: &[&str]) -> Self {
        Self {
            name: name.into(),
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            workdir: None,
            env: HashMap::new(),
            timeout: Duration::from_secs(120),
        }
    }
    pub fn with_workdir(mut self, d: impl Into<PathBuf>) -> Self {
        self.workdir = Some(d.into());
        self
    }
    pub fn with_env(mut self, k: &str, v: &str) -> Self {
        self.env.insert(k.into(), v.into());
        self
    }
}

#[async_trait]
impl SystemUnderTest for ProcessSystem {
    fn name(&self) -> &str {
        &self.name
    }
    async fn execute(&self, case: &ReplayCase) -> Result<Execution, ReplayError> {
        self.execute_batch(std::slice::from_ref(case))
            .await
            .pop()
            .unwrap()
    }
    async fn execute_batch(&self, cases: &[ReplayCase]) -> Vec<Result<Execution, ReplayError>> {
        let run = async {
            let mut cmd = Command::new(&self.program);
            cmd.args(&self.args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            if let Some(d) = &self.workdir {
                cmd.current_dir(d);
            }
            for (k, v) in &self.env {
                cmd.env(k, v);
            }
            let mut child = cmd
                .spawn()
                .map_err(|e| ReplayError::Sut(format!("spawn {}: {e}", self.program)))?;
            let mut stdin = child.stdin.take().unwrap();
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();
            let mut payload = String::new();
            for c in cases {
                payload.push_str(&serde_json::to_string(c).unwrap());
                payload.push('\n');
            }
            let writer = tokio::spawn(async move {
                let _ = stdin.write_all(payload.as_bytes()).await;
                let _ = stdin.shutdown().await;
            });
            let stderr_reader = tokio::spawn(async move {
                let mut bytes = Vec::new();
                let _ = stderr.take(64 * 1024).read_to_end(&mut bytes).await;
                String::from_utf8_lossy(&bytes).trim().to_string()
            });
            let mut lines = BufReader::new(stdout).lines();
            let mut outputs_by_id: HashMap<String, Result<Execution, ReplayError>> = HashMap::new();
            let mut sequential: VecDeque<Result<Execution, ReplayError>> = VecDeque::new();
            let started = Instant::now();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(&line) {
                    Ok(v) if v.get("error").is_some() => {
                        let error = Err(ReplayError::Sut(
                            v["error"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| v["error"].to_string()),
                        ));
                        if let Some(id) = v.get("id").and_then(Value::as_str) {
                            outputs_by_id.insert(id.to_string(), error);
                        } else {
                            sequential.push_back(error);
                        }
                    }
                    Ok(v) => {
                        let id = v.get("id").and_then(Value::as_str).map(str::to_owned);
                        let mut e: Execution = serde_json::from_value(v).unwrap_or_default();
                        if e.duration_ms == 0 {
                            e.duration_ms =
                                started.elapsed().as_millis() as u64 / cases.len().max(1) as u64;
                        }
                        if let Some(id) = id {
                            outputs_by_id.insert(id, Ok(e));
                        } else {
                            sequential.push_back(Ok(e));
                        }
                    }
                    Err(e) => sequential.push_back(Err(ReplayError::Sut(format!(
                        "bad output line: {e}: {line}"
                    )))),
                }
            }
            let _ = writer.await;
            let status = child
                .wait()
                .await
                .map_err(|e| ReplayError::Sut(e.to_string()))?;
            let stderr = stderr_reader.await.unwrap_or_default();
            if !status.success() && outputs_by_id.is_empty() && sequential.is_empty() {
                return Err(ReplayError::Sut(format!(
                    "{} exited with {status}: {stderr}",
                    self.program,
                )));
            }
            let outputs = cases
                .iter()
                .map(|case| {
                    outputs_by_id
                        .remove(&case.id)
                        .or_else(|| sequential.pop_front())
                        .unwrap_or_else(|| Err(ReplayError::Sut("missing output line".into())))
                })
                .collect();
            Ok::<_, ReplayError>(outputs)
        };
        let case_timeout = cases
            .iter()
            .filter_map(|case| case.options.timeout_ms)
            .min()
            .map(Duration::from_millis);
        let timeout = case_timeout.map_or(self.timeout, |value| value.min(self.timeout));
        match tokio::time::timeout(timeout, run).await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => cases.iter().map(|_| Err(e.clone())).collect(),
            Err(_) => cases
                .iter()
                .map(|_| Err(ReplayError::Sut("timeout".into())))
                .collect(),
        }
    }
}

/// HTTP adapter: `POST {url}` with the case JSON, expecting an [`Execution`] JSON body.
pub struct HttpSystem {
    pub name: String,
    pub url: String,
    http: reqwest::Client,
}

impl HttpSystem {
    pub fn new(name: &str, url: &str) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SystemUnderTest for HttpSystem {
    fn name(&self) -> &str {
        &self.name
    }
    async fn execute(&self, case: &ReplayCase) -> Result<Execution, ReplayError> {
        let started = Instant::now();
        let resp = self
            .http
            .post(&self.url)
            .json(&json!(case))
            .timeout(
                case.options
                    .timeout_ms
                    .map(Duration::from_millis)
                    .unwrap_or(Duration::from_secs(120)),
            )
            .send()
            .await
            .map_err(|e| ReplayError::Sut(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ReplayError::Sut(format!("HTTP {}", resp.status())));
        }
        let mut e: Execution = resp
            .json()
            .await
            .map_err(|e| ReplayError::Sut(e.to_string()))?;
        e.duration_ms = started.elapsed().as_millis() as u64;
        Ok(e)
    }
}
