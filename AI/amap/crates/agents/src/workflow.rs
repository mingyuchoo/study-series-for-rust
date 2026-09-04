//! The closed-loop modernization workflow (design §25):
//! discover → mine rules → mine behavior → uncertainty (HITL) → architecture → build →
//! [tests / boundary / adversarial] → verify → (RCA → fix → review → verify)* → certify.
use crate::*;
use amap_domain::GateReport;
use amap_orchestrator::{AgentContext, Dag, Executor, RunReport};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowOutcome {
    pub reports: Vec<RunReport>,
    pub certified: bool,
    pub gate: Option<GateReport>,
    pub iterations: u32,
    pub halted: Option<String>,
    pub failed: Option<String>,
    pub final_outputs: WorkflowState,
}

pub fn phase_one() -> Dag {
    Dag::new()
        .add(Arc::new(DiscoveryAgent), &[])
        .add_with_retries(Arc::new(RuleMinerAgent), &["discovery"], 2)
        .add(Arc::new(BehaviorMinerAgent), &["rule_mining"])
        .add(Arc::new(UncertaintyAgent), &["behavior_mining"])
}

/// Work that is safe to resume after an explicit HITL decision.
pub fn phase_two() -> Dag {
    Dag::new()
        .add_with_retries(Arc::new(ArchitectureAgent), &[], 2)
        .add_with_retries(Arc::new(BuilderAgent::default()), &["architecture"], 2)
        .add(Arc::new(TestGeneratorAgent), &[])
        .add(Arc::new(BoundaryAgent), &[])
        .add(Arc::new(AdversarialAgent), &["build"])
}

pub fn verify_dag() -> Dag {
    Dag::new().add(Arc::new(VerifierAgent::default()), &[])
}

pub fn repair_dag() -> Dag {
    Dag::new()
        .add_with_retries(Arc::new(RcaAgent::default()), &[], 2)
        .add_with_retries(Arc::new(FixAgent::default()), &["rca"], 2)
        .add(Arc::new(ReviewAgent::default()), &["fix"])
}

fn serialization_error(error: serde_json::Error) -> amap_orchestrator::OrchestrationError {
    amap_orchestrator::OrchestrationError::Other(format!("workflow state: {error}"))
}

fn sync_context(
    ctx: &mut AgentContext,
    state: &WorkflowState,
) -> Result<(), amap_orchestrator::OrchestrationError> {
    ctx.inputs = state.to_value().map_err(serialization_error)?;
    Ok(())
}

fn merge_report(
    ctx: &mut AgentContext,
    state: &mut WorkflowState,
    report: &RunReport,
) -> Result<(), amap_orchestrator::OrchestrationError> {
    state
        .merge_value(report.outputs.clone())
        .map_err(serialization_error)?;
    sync_context(ctx, state)
}

pub async fn run_modernization(
    mut ctx: AgentContext,
) -> Result<WorkflowOutcome, amap_orchestrator::OrchestrationError> {
    let mut state = WorkflowState::from_value(ctx.inputs.clone()).map_err(serialization_error)?;
    sync_context(&mut ctx, &state)?;
    let mut reports = Vec::new();
    let mut outcome = WorkflowOutcome {
        reports: vec![],
        certified: false,
        gate: None,
        iterations: 0,
        halted: None,
        failed: None,
        final_outputs: WorkflowState::default(),
    };

    if state.uncertainty.is_none() {
        let r = Executor::run(&phase_one(), ctx.clone()).await?;
        merge_report(&mut ctx, &mut state, &r)?;
        let halted = r.halted.clone();
        let failed = r.failed.clone();
        reports.push(r);
        if halted.is_some() || failed.is_some() {
            outcome.reports = reports;
            outcome.halted = halted;
            outcome.failed = failed;
            outcome.final_outputs = state;
            return Ok(outcome);
        }
    }

    let r = Executor::run(&phase_two(), ctx.clone()).await?;
    merge_report(&mut ctx, &mut state, &r)?;
    let failed = r.failed.clone();
    reports.push(r);
    if let Some(failed) = failed {
        outcome.reports = reports;
        outcome.failed = Some(failed);
        outcome.final_outputs = state;
        return Ok(outcome);
    }

    let mut iteration = 0;
    loop {
        let r = Executor::run(&verify_dag(), ctx.clone()).await?;
        merge_report(&mut ctx, &mut state, &r)?;
        let failed = r.failed.clone();
        reports.push(r);
        if let Some(f) = failed {
            outcome.failed = Some(f);
            break;
        }
        let verify = state.verify.as_ref().ok_or_else(|| {
            amap_orchestrator::OrchestrationError::Other(
                "verification completed without a typed output".into(),
            )
        })?;
        outcome.gate = Some(verify.gate.clone());
        if verify.certified {
            outcome.certified = true;
            break;
        }
        let blocked: Vec<String> = outcome
            .gate
            .as_ref()
            .map(|g| {
                g.checks
                    .iter()
                    .filter(|c| !c.passed)
                    .map(|c| c.kpi.clone())
                    .collect()
            })
            .unwrap_or_default();
        if verify.failures.is_empty() {
            outcome.failed = Some(format!("gate blocked by {:?} with no unexplained functional differences — strengthen verification (more golden/boundary/adversarial scenarios), not the code", blocked));
            break;
        }
        if iteration >= ctx.config.max_fix_iterations {
            outcome.failed = Some(format!(
                "gate still failing after {iteration} repair iteration(s)"
            ));
            break;
        }
        iteration += 1;
        let r = Executor::run(&repair_dag(), ctx.clone()).await?;
        merge_report(&mut ctx, &mut state, &r)?;
        let failed = r.failed.clone();
        let approved = state
            .review
            .as_ref()
            .map(|review| review.approved)
            .unwrap_or(false);
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
    outcome.final_outputs = state;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordered_names(dag: &Dag) -> Vec<String> {
        dag.order()
            .unwrap()
            .into_iter()
            .map(|index| dag.nodes[index].name.clone())
            .collect()
    }

    #[test]
    fn workflow_phase_topology_is_stable() {
        assert_eq!(
            ordered_names(&phase_one()),
            ["discovery", "rule_mining", "behavior_mining", "uncertainty"]
        );
        assert_eq!(
            ordered_names(&phase_two()),
            [
                "architecture",
                "test_generation",
                "boundary",
                "build",
                "adversarial",
            ]
        );
        assert_eq!(ordered_names(&verify_dag()), ["verify"]);
        assert_eq!(ordered_names(&repair_dag()), ["rca", "fix", "review"]);
    }

    #[test]
    fn merging_step_outputs_preserves_unrelated_steps_and_replaces_same_step() {
        let mut state = WorkflowState {
            discovery: Some(DiscoveryOutput {
                entities: 3,
                ..Default::default()
            }),
            review: Some(ReviewOutput::default()),
            ..Default::default()
        };

        state
            .merge_value(
                serde_json::to_value(WorkflowState {
                    review: Some(ReviewOutput {
                        approved: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .unwrap(),
            )
            .unwrap();

        assert_eq!(state.discovery.unwrap().entities, 3);
        assert!(state.review.unwrap().approved);
    }
}
