//! DAG executor: topological execution with retries, event publication and HITL halts.
use crate::{AgentContext, AgentTask, OrchestrationError};
use amap_domain::AgentRole;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tracing::Instrument;

pub struct DagNode {
    pub name: String,
    pub task: Arc<dyn AgentTask>,
    pub deps: Vec<String>,
    pub retries: u32,
}

#[derive(Default)]
pub struct Dag {
    pub nodes: Vec<DagNode>,
}

impl Dag {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(mut self, task: Arc<dyn AgentTask>, deps: &[&str]) -> Self {
        self.nodes.push(DagNode {
            name: task.name().to_string(),
            task,
            deps: deps.iter().map(|d| d.to_string()).collect(),
            retries: 1,
        });
        self
    }
    pub fn add_with_retries(
        mut self,
        task: Arc<dyn AgentTask>,
        deps: &[&str],
        retries: u32,
    ) -> Self {
        self.nodes.push(DagNode {
            name: task.name().to_string(),
            task,
            deps: deps.iter().map(|d| d.to_string()).collect(),
            retries,
        });
        self
    }

    /// Kahn topological order.
    pub fn order(&self) -> Result<Vec<usize>, OrchestrationError> {
        let index: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.name.as_str(), i))
            .collect();
        let mut indeg = vec![0usize; self.nodes.len()];
        let mut adj: Vec<Vec<usize>> = vec![vec![]; self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            for d in &n.deps {
                let &j = index.get(d.as_str()).ok_or_else(|| {
                    OrchestrationError::InvalidDag(format!("{} depends on unknown {d}", n.name))
                })?;
                adj[j].push(i);
                indeg[i] += 1;
            }
        }
        let mut ready: Vec<usize> = (0..self.nodes.len()).filter(|&i| indeg[i] == 0).collect();
        let mut out = Vec::new();
        while let Some(i) = ready.first().copied() {
            ready.remove(0);
            out.push(i);
            for &j in &adj[i] {
                indeg[j] -= 1;
                if indeg[j] == 0 {
                    ready.push(j);
                }
            }
        }
        if out.len() != self.nodes.len() {
            return Err(OrchestrationError::InvalidDag("cycle".into()));
        }
        Ok(out)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Succeeded,
    Failed,
    Halted,
    Skipped,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepReport {
    pub name: String,
    pub role: AgentRole,
    pub status: StepStatus,
    pub attempts: u32,
    pub summary: String,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunReport {
    pub run_id: String,
    pub function_id: String,
    pub steps: Vec<StepReport>,
    pub outputs: Value,
    pub halted: Option<String>,
    pub failed: Option<String>,
}

impl RunReport {
    pub fn succeeded(&self) -> bool {
        self.halted.is_none() && self.failed.is_none()
    }
}

pub struct Executor;

async fn checkpoint(
    ctx: &AgentContext,
    outputs: &serde_json::Map<String, Value>,
    status: Option<&str>,
) -> Result<(), OrchestrationError> {
    if let Some(mut run) = ctx.knowledge.get_workflow_run(&ctx.run_id.0).await? {
        run.checkpoint = Value::Object(outputs.clone());
        run.updated_at = ctx.clock.now();
        if let Some(status) = status {
            run.status = status.to_string();
        }
        ctx.knowledge.upsert_workflow_run(run).await?;
    }
    Ok(())
}

impl Executor {
    /// Run the DAG sequentially in topological order (agents are heavyweight; parallelism happens inside engines / workers).
    pub async fn run(dag: &Dag, base: AgentContext) -> Result<RunReport, OrchestrationError> {
        let order = dag.order()?;
        let mut outputs = base.inputs.as_object().cloned().unwrap_or_default();
        let mut report = RunReport {
            run_id: base.run_id.0.clone(),
            function_id: base.function_id.0.clone(),
            steps: vec![],
            outputs: Value::Null,
            halted: None,
            failed: None,
        };
        let mut failed_nodes: HashSet<String> = HashSet::new();

        for i in order {
            let node = &dag.nodes[i];
            if node.deps.iter().any(|d| failed_nodes.contains(d)) {
                report.steps.push(StepReport {
                    name: node.name.clone(),
                    role: node.task.role(),
                    status: StepStatus::Skipped,
                    attempts: 0,
                    summary: "upstream failed".into(),
                    duration_ms: 0,
                });
                failed_nodes.insert(node.name.clone());
                continue;
            }
            let mut ctx = base.clone();
            ctx.inputs = Value::Object(outputs.clone());
            ctx.emit(&format!("agent.{}.started", node.name), json!({}))
                .await;
            let started = Instant::now();
            let mut attempts = 0;
            let result = loop {
                attempts += 1;
                let span = tracing::info_span!(
                    "agent.execute",
                    run_id = %base.run_id,
                    function_id = %base.function_id,
                    agent = %node.name,
                    attempt = attempts,
                );
                match node.task.execute(&ctx).instrument(span).await {
                    Ok(r) => break Ok(r),
                    Err(e) if attempts < node.retries.max(1) => {
                        tracing::warn!(agent = %node.name, attempt = attempts, error = %e, "agent failed; retrying");
                    }
                    Err(e) => break Err(e),
                }
            };
            let duration_ms = started.elapsed().as_millis() as u64;
            match result {
                Ok(r) => {
                    let payload_uri = if r.evidence.is_empty() {
                        None
                    } else {
                        Some(ctx.lake.append(&r.evidence).await?)
                    };
                    for ev in &r.evidence {
                        let mut linked = ev.clone();
                        linked.payload_uri = payload_uri.clone();
                        ctx.knowledge.record_evidence(linked).await?;
                    }
                    outputs.insert(node.name.clone(), r.outputs.clone());
                    checkpoint(&ctx, &outputs, r.halt.as_ref().map(|_| "halted")).await?;
                    if let Some(reason) = r.halt {
                        ctx.emit(
                            &format!("agent.{}.halted", node.name),
                            json!({ "reason": reason }),
                        )
                        .await;
                        report.steps.push(StepReport {
                            name: node.name.clone(),
                            role: node.task.role(),
                            status: StepStatus::Halted,
                            attempts,
                            summary: r.summary,
                            duration_ms,
                        });
                        report.halted = Some(reason);
                        break;
                    }
                    ctx.emit(
                        &format!("agent.{}.completed", node.name),
                        json!({ "summary": r.summary }),
                    )
                    .await;
                    tracing::info!(agent = %node.name, ms = duration_ms, "{}", r.summary);
                    report.steps.push(StepReport {
                        name: node.name.clone(),
                        role: node.task.role(),
                        status: StepStatus::Succeeded,
                        attempts,
                        summary: r.summary,
                        duration_ms,
                    });
                }
                Err(e) => {
                    ctx.emit(
                        &format!("agent.{}.failed", node.name),
                        json!({ "error": e.to_string() }),
                    )
                    .await;
                    tracing::error!(agent = %node.name, error = %e, "agent failed");
                    report.steps.push(StepReport {
                        name: node.name.clone(),
                        role: node.task.role(),
                        status: StepStatus::Failed,
                        attempts,
                        summary: e.to_string(),
                        duration_ms,
                    });
                    failed_nodes.insert(node.name.clone());
                    report.failed = Some(format!("{}: {e}", node.name));
                    checkpoint(&ctx, &outputs, Some("failed")).await?;
                }
            }
        }
        report.outputs = Value::Object(outputs);
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentResult, InMemoryBus, RunConfig, SystemClock, UuidGenerator};
    use amap_domain::*;
    use async_trait::async_trait;

    struct T(&'static str, Vec<&'static str>);
    #[async_trait]
    impl AgentTask for T {
        fn name(&self) -> &str {
            self.0
        }
        fn role(&self) -> AgentRole {
            AgentRole::Discovery
        }
        async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
            for d in &self.1 {
                assert!(ctx.inputs.get(d).is_some(), "missing upstream {d}");
            }
            Ok(AgentResult::new(
                format!("{} done", self.0),
                json!({"ok": true}),
            ))
        }
    }

    #[tokio::test]
    async fn runs_in_dependency_order() {
        let dag = Dag::new()
            .add(Arc::new(T("c", vec!["a", "b"])), &["a", "b"])
            .add(Arc::new(T("a", vec![])), &[])
            .add(Arc::new(T("b", vec!["a"])), &["a"]);
        let lake = Arc::new(
            amap_evidence::EvidenceLake::open_local(
                std::env::temp_dir().join(format!("amap-{}", uuid::Uuid::new_v4())),
            )
            .await
            .unwrap(),
        );
        let ctx = AgentContext {
            run_id: RunId::new("RUN-t"),
            function_id: FunctionId::new("FN-t"),
            knowledge: Arc::new(amap_knowledge::InMemoryKnowledgeStore::new()),
            llm: Arc::new(amap_llm::MockProvider::new()),
            bus: Arc::new(InMemoryBus::new()),
            lake,
            policy: Arc::new(amap_policy::PolicyEngine::default()),
            clock: Arc::new(SystemClock),
            ids: Arc::new(UuidGenerator),
            config: Arc::new(RunConfig {
                source_root: ".".into(),
                documents: vec![],
                traces: None,
                trace_sources: vec![],
                workspace: ".".into(),
                next_command: vec![],
                legacy_command: None,
                comparator_specs: vec![],
                default_spec: None,
                invariants: None,
                timing_tolerance_ms: None,
                max_fix_iterations: 1,
                max_mutants: 1,
                faults: vec![],
                concurrency_template: None,
                thresholds: Default::default(),
                auto_approve_hitl: true,
                verifier_endpoint: None,
                verifier_token: None,
                verifier_tls_ca: None,
                verifier_tls_cert: None,
                verifier_tls_key: None,
                verifier_tls_domain: None,
                verifier_allow_insecure: true,
                verifier_artifact_max_bytes: 0,
                token_budget: 0,
            }),
            inputs: json!({}),
        };
        let report = Executor::run(&dag, ctx).await.unwrap();
        let names: Vec<_> = report.steps.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
        assert!(report.succeeded());
    }
}
