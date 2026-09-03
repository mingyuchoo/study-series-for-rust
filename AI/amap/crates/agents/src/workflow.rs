//! The closed-loop modernization workflow (design §25):
//! discover → mine rules → mine behavior → uncertainty (HITL) → architecture → build →
//! [tests / boundary / adversarial] → verify → (RCA → fix → review → verify)* → certify.
use crate::*;
use amap_domain::GateReport;
use amap_orchestrator::{AgentContext, Dag, Executor, RunReport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowOutcome {
    pub reports: Vec<RunReport>,
    pub certified: bool,
    pub gate: Option<GateReport>,
    pub iterations: u32,
    pub halted: Option<String>,
    pub failed: Option<String>,
    pub final_outputs: Value,
}

pub fn phase_one() -> Dag {
    Dag::new()
        .add(Arc::new(DiscoveryAgent), &[])
        .add_with_retries(Arc::new(RuleMinerAgent), &["discovery"], 2)
        .add(Arc::new(BehaviorMinerAgent), &["rule_mining"])
        .add(Arc::new(UncertaintyAgent), &["behavior_mining"])
        .add_with_retries(Arc::new(ArchitectureAgent), &["uncertainty"], 2)
        .add_with_retries(Arc::new(BuilderAgent), &["architecture"], 2)
        .add(Arc::new(TestGeneratorAgent), &["rule_mining"])
        .add(Arc::new(BoundaryAgent), &["behavior_mining"])
        .add(Arc::new(AdversarialAgent), &["build"])
}

pub fn verify_dag() -> Dag {
    Dag::new().add(Arc::new(VerifierAgent), &[])
}

pub fn repair_dag() -> Dag {
    Dag::new().add_with_retries(Arc::new(RcaAgent), &[], 2).add_with_retries(Arc::new(FixAgent), &["rca"], 2).add(Arc::new(ReviewAgent), &["fix"])
}

fn merge(base: &mut Value, outputs: &Value) {
    if let (Value::Object(b), Value::Object(o)) = (base, outputs) {
        for (k, v) in o {
            b.insert(k.clone(), v.clone());
        }
    }
}

pub async fn run_modernization(mut ctx: AgentContext) -> Result<WorkflowOutcome, amap_orchestrator::OrchestrationError> {
    if ctx.inputs.is_null() {
        ctx.inputs = serde_json::json!({});
    }
    let mut reports = Vec::new();
    let mut outcome = WorkflowOutcome { reports: vec![], certified: false, gate: None, iterations: 0, halted: None, failed: None, final_outputs: Value::Null };

    let r = Executor::run(&phase_one(), ctx.clone()).await?;
    merge(&mut ctx.inputs, &r.outputs);
    let halted = r.halted.clone();
    let failed = r.failed.clone();
    reports.push(r);
    if halted.is_some() || failed.is_some() {
        outcome.reports = reports;
        outcome.halted = halted;
        outcome.failed = failed;
        outcome.final_outputs = ctx.inputs;
        return Ok(outcome);
    }

    let mut iteration = 0;
    loop {
        let r = Executor::run(&verify_dag(), ctx.clone()).await?;
        merge(&mut ctx.inputs, &r.outputs);
        let failed = r.failed.clone();
        reports.push(r);
        if let Some(f) = failed {
            outcome.failed = Some(f);
            break;
        }
        let verify = ctx.inputs["verify"].clone();
        outcome.gate = serde_json::from_value(verify["gate"].clone()).ok();
        if verify["certified"].as_bool().unwrap_or(false) {
            outcome.certified = true;
            break;
        }
        let blocked: Vec<String> = outcome.gate.as_ref().map(|g| g.checks.iter().filter(|c| !c.passed).map(|c| c.kpi.clone()).collect()).unwrap_or_default();
        if verify["failures"].as_array().map(|a| a.is_empty()).unwrap_or(true) {
            outcome.failed = Some(format!("gate blocked by {:?} with no unexplained functional differences — strengthen verification (more golden/boundary/adversarial scenarios), not the code", blocked));
            break;
        }
        if iteration >= ctx.config.max_fix_iterations {
            outcome.failed = Some(format!("gate still failing after {iteration} repair iteration(s)"));
            break;
        }
        iteration += 1;
        let r = Executor::run(&repair_dag(), ctx.clone()).await?;
        merge(&mut ctx.inputs, &r.outputs);
        let failed = r.failed.clone();
        let approved = ctx.inputs["review"]["approved"].as_bool().unwrap_or(false);
        reports.push(r);
        if let Some(f) = failed {
            outcome.failed = Some(f);
            break;
        }
        if !approved {
            outcome.failed = Some("independent review rejected the patch".into());
            break;
        }
    }
    outcome.iterations = iteration;
    outcome.reports = reports;
    outcome.final_outputs = ctx.inputs;
    Ok(outcome)
}
