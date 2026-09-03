//! Verification engines shared by the in-process verifier and the gRPC workers.
pub mod concurrency;
pub mod fault;
pub mod mutation;
pub mod static_check;

use amap_comparator::ComparatorSpec;
use amap_domain::*;
use amap_orchestrator::RunConfig;
use amap_replay::{ExecOptions, ProcessSystem, ReplayEngine, SystemUnderTest};
use serde_json::{json, Value};

pub fn build_engine(cfg: &RunConfig) -> Result<ReplayEngine, String> {
    let mut engine = ReplayEngine::new();
    for p in &cfg.comparator_specs {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        engine = engine.with_spec(ComparatorSpec::from_yaml(&text).map_err(|e| e.to_string())?);
    }
    if let Some(p) = &cfg.invariants {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        engine = engine.with_invariants(amap_invariant::parse(&text).map_err(|e| e.to_string())?);
    }
    Ok(engine)
}

pub fn process_from(cfg: &RunConfig, name: &str, cmd: &[String]) -> Option<ProcessSystem> {
    let resolved = cfg.resolved_command(cmd);
    let (program, args) = resolved.split_first()?;
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    Some(ProcessSystem::new(name, program, &args))
}

pub fn next_system(cfg: &RunConfig) -> Result<ProcessSystem, String> {
    process_from(cfg, "next", &cfg.next_command).ok_or_else(|| "next_command is empty".to_string())
}

pub fn legacy_oracle(cfg: &RunConfig) -> Option<ProcessSystem> {
    cfg.legacy_command.as_ref().and_then(|c| process_from(cfg, "legacy", c))
}

/// Run one verification engine. Returns results and engine metrics.
pub async fn run_engine(kind: VerificationKind, scenarios: &[TestScenario], cfg: &RunConfig, function_id: &FunctionId) -> Result<(Vec<VerificationResult>, Value), String> {
    match kind {
        VerificationKind::Static => Ok((static_check::run(cfg, function_id), json!({}))),
        VerificationKind::GoldenReplay | VerificationKind::Differential | VerificationKind::Boundary | VerificationKind::Adversarial | VerificationKind::Property | VerificationKind::State | VerificationKind::Unit | VerificationKind::Interface => {
            let engine = build_engine(cfg)?;
            let next = next_system(cfg)?;
            let oracle = legacy_oracle(cfg);
            let results = engine
                .replay(&next, oracle.as_ref().map(|o| o as &dyn SystemUnderTest), scenarios, kind, &ExecOptions::default())
                .await
                .map_err(|e| e.to_string())?;
            Ok((results, json!({ "oracle": oracle.is_some() })))
        }
        VerificationKind::Mutation => mutation::run(scenarios, cfg, function_id).await,
        VerificationKind::Fault => fault::run(scenarios, cfg, function_id).await,
        VerificationKind::Concurrency => concurrency::run(cfg, function_id).await,
    }
}
