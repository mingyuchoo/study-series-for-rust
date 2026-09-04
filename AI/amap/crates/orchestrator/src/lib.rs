//! Agent orchestration: the [`AgentTask`] abstraction, a DAG executor with retries and HITL halts,
//! an event bus (in-memory or NATS JetStream), and the gRPC contract for verification workers.
//!
//! The workflow abstraction is deliberately independent of Temporal so durable execution can be
//! delegated later without rewriting agents (stack §2).

pub mod bus;
pub mod config;
pub mod dag;
pub mod ports;

pub mod proto {
    tonic::include_proto!("amap.v1");
}

#[cfg(feature = "nats")]
pub use bus::NatsBus;
pub use bus::{Event, EventBus, InMemoryBus};
pub use config::{ComparatorPlugin, RunConfig, TraceSource};
pub use dag::{Dag, DagNode, Executor, RunReport, StepReport, StepStatus};
pub use ports::{Clock, IdGenerator, SystemClock, UuidGenerator};

use amap_domain::*;
use amap_evidence::EvidenceLake;
use amap_knowledge::KnowledgeStore;
use amap_llm::LlmClient;
use amap_policy::PolicyEngine;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum OrchestrationError {
    #[error("agent `{agent}` failed: {message}")]
    Agent { agent: String, message: String },
    #[error("policy denied {action} for {principal}: {reasons:?}")]
    PolicyDenied {
        principal: String,
        action: String,
        reasons: Vec<String>,
    },
    #[error("dependency cycle or unknown dependency: {0}")]
    InvalidDag(String),
    #[error(transparent)]
    Knowledge(#[from] amap_knowledge::KnowledgeError),
    #[error(transparent)]
    Llm(#[from] amap_llm::LlmError),
    #[error(transparent)]
    Evidence(#[from] amap_evidence::EvidenceError),
    #[error(transparent)]
    Policy(#[from] amap_policy::PolicyError),
    #[error("{0}")]
    Other(String),
}

impl OrchestrationError {
    pub fn agent(agent: &str, e: impl std::fmt::Display) -> Self {
        Self::Agent {
            agent: agent.into(),
            message: e.to_string(),
        }
    }
}

/// Everything an agent may touch. Agents get a *bounded* view of the world through these handles.
#[derive(Clone)]
pub struct AgentContext {
    pub run_id: RunId,
    pub function_id: FunctionId,
    pub knowledge: Arc<dyn KnowledgeStore>,
    pub llm: Arc<dyn LlmClient>,
    pub bus: Arc<dyn EventBus>,
    pub lake: Arc<EvidenceLake>,
    pub policy: Arc<PolicyEngine>,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdGenerator>,
    pub config: Arc<RunConfig>,
    /// Outputs of upstream tasks, keyed by task name.
    pub inputs: Value,
}

impl AgentContext {
    pub fn input(&self, task: &str) -> Value {
        self.inputs.get(task).cloned().unwrap_or(Value::Null)
    }

    pub fn input_as<T: DeserializeOwned>(
        &self,
        task: &str,
    ) -> Result<Option<T>, OrchestrationError> {
        let value = self.input(task);
        if value.is_null() {
            return Ok(None);
        }
        serde_json::from_value(value)
            .map(Some)
            .map_err(|error| OrchestrationError::Other(format!("invalid {task} output: {error}")))
    }

    /// Enforce a governance policy before an agent acts.
    pub fn authorize(
        &self,
        principal: &amap_policy::Principal,
        action: &str,
        resource: &str,
        ctx: &amap_policy::ActionContext,
    ) -> Result<(), OrchestrationError> {
        let d = self.policy.authorize(principal, action, resource, ctx)?;
        if d.allowed {
            Ok(())
        } else {
            Err(OrchestrationError::PolicyDenied {
                principal: principal.id.clone(),
                action: action.into(),
                reasons: d.reasons,
            })
        }
    }

    pub async fn emit(&self, subject: &str, payload: Value) {
        self.bus
            .publish(Event {
                subject: subject.to_string(),
                run_id: self.run_id.0.clone(),
                function_id: self.function_id.0.clone(),
                payload,
                at: self.clock.now(),
            })
            .await;
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentResult {
    pub outputs: Value,
    pub summary: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceRecord>,
    /// Set to halt the DAG (e.g. mandatory SME decision pending).
    #[serde(default)]
    pub halt: Option<String>,
}

impl AgentResult {
    pub fn new(summary: impl Into<String>, outputs: Value) -> Self {
        Self {
            outputs,
            summary: summary.into(),
            evidence: vec![],
            halt: None,
        }
    }
    pub fn halt(summary: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            outputs: Value::Null,
            summary: summary.into(),
            evidence: vec![],
            halt: Some(reason.into()),
        }
    }

    pub fn typed<T: Serialize>(
        summary: impl Into<String>,
        output: T,
    ) -> Result<Self, OrchestrationError> {
        let outputs = serde_json::to_value(output)
            .map_err(|error| OrchestrationError::Other(error.to_string()))?;
        Ok(Self::new(summary, outputs))
    }
}

#[async_trait]
pub trait AgentTask: Send + Sync {
    fn name(&self) -> &str;
    fn role(&self) -> AgentRole;
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError>;
}
