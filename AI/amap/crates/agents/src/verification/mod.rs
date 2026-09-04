//! Verification engines shared by the in-process verifier and the gRPC workers.
pub mod concurrency;
pub mod fault;
pub mod mutation;
pub mod static_check;

use amap_comparator::wasm::WasmComparator;
use amap_comparator::{ComparatorKind, ComparatorSpec};
use amap_domain::*;
use amap_orchestrator::RunConfig;
use amap_replay::{ExecOptions, ProcessSystem, ReplayEngine, SystemUnderTest};
use serde_json::{json, Value};
use std::sync::Arc;

/// Build the equivalence engine for a run: WASM plugins first, then the specs that reference them,
/// then business invariants. A spec naming an unregistered plugin fails here, before any replay.
pub fn build_engine(cfg: &RunConfig) -> Result<ReplayEngine, String> {
    let mut engine = ReplayEngine::new().with_timing_tolerance(cfg.timing_tolerance_ms);
    for plugin in &cfg.comparator_plugins {
        let comparator = WasmComparator::from_file(&plugin.name, &plugin.path)
            .map_err(|e| format!("comparator plugin `{}`: {e}", plugin.name))?;
        engine
            .comparator
            .register_plugin(plugin.name.clone(), Arc::new(comparator));
    }
    let registered = engine.comparator.plugin_names();
    for p in &cfg.comparator_specs {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let spec = ComparatorSpec::from_yaml(&text).map_err(|e| e.to_string())?;
        for (path, rule) in &spec.fields {
            if rule.comparator == ComparatorKind::Plugin {
                let name = rule.plugin.as_deref().unwrap_or_default();
                if !registered.iter().any(|r| r == name) {
                    return Err(format!(
                        "{}: field `{path}` uses comparator plugin `{name}` which is not declared in [[run.comparator_plugins]]",
                        p.display()
                    ));
                }
            }
        }
        engine = engine.with_spec(spec);
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
    cfg.legacy_command
        .as_ref()
        .and_then(|c| process_from(cfg, "legacy", c))
}

/// Run one verification engine. Returns results and engine metrics.
pub async fn run_engine(
    kind: VerificationKind,
    scenarios: &[TestScenario],
    cfg: &RunConfig,
    function_id: &FunctionId,
) -> Result<(Vec<VerificationResult>, Value), String> {
    match kind {
        VerificationKind::Static => Ok((static_check::run(cfg, function_id), json!({}))),
        VerificationKind::GoldenReplay
        | VerificationKind::Differential
        | VerificationKind::Boundary
        | VerificationKind::Adversarial
        | VerificationKind::Property
        | VerificationKind::State
        | VerificationKind::Unit
        | VerificationKind::Interface => {
            let engine = build_engine(cfg)?;
            let next = next_system(cfg)?;
            let oracle = if kind == VerificationKind::Unit {
                None
            } else {
                legacy_oracle(cfg)
            };
            let results = engine
                .replay(
                    &next,
                    oracle.as_ref().map(|o| o as &dyn SystemUnderTest),
                    scenarios,
                    kind,
                    &ExecOptions::default(),
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok((results, json!({ "oracle": oracle.is_some() })))
        }
        VerificationKind::Mutation => mutation::run(scenarios, cfg, function_id).await,
        VerificationKind::Fault => fault::run(scenarios, cfg, function_id).await,
        VerificationKind::Concurrency => concurrency::run(cfg, function_id).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amap_orchestrator::ComparatorPlugin;
    use std::path::PathBuf;

    /// Minimal WAT plugin: equal iff both JSON payloads have the same byte length.
    const WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 1024))
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $p))
  (func (export "compare") (param i32 i32 i32 i32) (result i32)
    (if (result i32) (i32.eq (local.get 1) (local.get 3)) (then (i32.const 0)) (else (i32.const 1)))))
"#;

    const SPEC: &str =
        "name: plugged\nfields:\n  amount: { comparator: plugin, plugin: same_len }\n";

    fn config(dir: &std::path::Path, plugins: Vec<ComparatorPlugin>) -> RunConfig {
        RunConfig {
            source_root: dir.to_path_buf(),
            documents: vec![],
            traces: None,
            trace_sources: vec![],
            workspace: dir.to_path_buf(),
            next_command: vec![],
            legacy_command: None,
            comparator_specs: vec![dir.join("spec.yaml")],
            comparator_plugins: plugins,
            default_spec: Some("plugged".into()),
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
        }
    }

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("amap-plugin-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("spec.yaml"), SPEC).unwrap();
        std::fs::write(dir.join("same_len.wat"), WAT).unwrap();
        dir
    }

    #[test]
    fn run_config_plugins_are_registered_and_used_by_specs() {
        let dir = scratch();
        let cfg = config(
            &dir,
            vec![ComparatorPlugin {
                name: "same_len".into(),
                path: dir.join("same_len.wat"),
            }],
        );
        let engine = build_engine(&cfg).unwrap();
        assert_eq!(
            engine.comparator.plugin_names(),
            vec!["same_len".to_string()]
        );
        let spec = engine.specs.get("plugged").unwrap();
        let equal = engine
            .comparator
            .compare(spec, &json!({"amount": 12}), &json!({"amount": 34}))
            .unwrap();
        assert!(equal.equal, "{:?}", equal.differences);
        let different = engine
            .comparator
            .compare(spec, &json!({"amount": 12}), &json!({"amount": 345}))
            .unwrap();
        assert!(!different.equal);
        assert_eq!(different.differences[0].comparator, "same_len");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn spec_referencing_undeclared_plugin_fails_before_replay() {
        let dir = scratch();
        let error = build_engine(&config(&dir, vec![]))
            .err()
            .expect("undeclared plugin must fail");
        assert!(error.contains("same_len"), "{error}");
        assert!(error.contains("comparator_plugins"), "{error}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_plugin_file_is_reported_by_name() {
        let dir = scratch();
        let cfg = config(
            &dir,
            vec![ComparatorPlugin {
                name: "same_len".into(),
                path: dir.join("nope.wasm"),
            }],
        );
        let error = build_engine(&cfg)
            .err()
            .expect("missing plugin file must fail");
        assert!(error.starts_with("comparator plugin `same_len`"), "{error}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
