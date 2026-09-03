//! Verification Factory runner (design §9): runs every independent engine, writes evidence,
//! and computes the quality gate deterministically. Remote workers are used when configured.
use crate::evidence_from_results;
use crate::verification::run_engine;
use amap_domain::*;
use amap_evidence::{build_certificate, CertificateInputs};
use amap_orchestrator::proto::verification_client::VerificationClient;
use amap_orchestrator::proto::VerifyRequest;
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Default)]
pub struct VerifierAgent;

async fn dispatch(ctx: &AgentContext, kind: VerificationKind, scenarios: &[TestScenario]) -> Result<(Vec<VerificationResult>, Value), OrchestrationError> {
    if let Some(endpoint) = &ctx.config.verifier_endpoint {
        let mut client = VerificationClient::connect(endpoint.clone()).await.map_err(|e| OrchestrationError::Other(format!("worker connect: {e}")))?;
        let req = VerifyRequest {
            run_id: ctx.run_id.0.clone(),
            function_id: ctx.function_id.0.clone(),
            kind: serde_json::to_value(kind).unwrap().as_str().unwrap().to_string(),
            scenarios_json: serde_json::to_vec(scenarios).unwrap(),
            config_json: serde_json::to_vec(&*ctx.config).unwrap(),
        };
        let resp = client.verify(req).await.map_err(|e| OrchestrationError::Other(format!("worker verify: {e}")))?.into_inner();
        let results: Vec<VerificationResult> = serde_json::from_slice(&resp.results_json).map_err(|e| OrchestrationError::Other(e.to_string()))?;
        let metrics: Value = serde_json::from_slice(&resp.metrics_json).unwrap_or(json!({}));
        return Ok((results, metrics));
    }
    run_engine(kind, scenarios, &ctx.config, &ctx.function_id).await.map_err(|e| OrchestrationError::agent("verify", e))
}

#[async_trait]
impl AgentTask for VerifierAgent {
    fn name(&self) -> &str {
        "verify"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Replay
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let scenarios = ctx.knowledge.scenarios_for(&ctx.function_id).await?;
        let golden: Vec<TestScenario> = scenarios.iter().filter(|s| s.origin == ScenarioOrigin::GoldenMaster).cloned().collect();
        let derived: Vec<TestScenario> = scenarios.iter().filter(|s| s.origin != ScenarioOrigin::GoldenMaster).cloned().collect();
        let mut all_results: Vec<VerificationResult> = Vec::new();
        let mut metrics = serde_json::Map::new();

        // 1. Static
        let (r, m) = dispatch(ctx, VerificationKind::Static, &[]).await?;
        metrics.insert("static".into(), m);
        all_results.extend(r);

        // 2. Golden replay (production behaviors as oracle)
        let (r, m) = dispatch(ctx, VerificationKind::GoldenReplay, &golden).await?;
        metrics.insert("golden_replay".into(), m);
        all_results.extend(r);

        // 3. Differential (boundary / property / adversarial vs legacy oracle) + invariants
        let mut differential: Vec<VerificationResult> = Vec::new();
        for (origin, kind) in [(ScenarioOrigin::Boundary, VerificationKind::Boundary), (ScenarioOrigin::Property, VerificationKind::Property), (ScenarioOrigin::Adversarial, VerificationKind::Adversarial)] {
            let subset: Vec<TestScenario> = derived.iter().filter(|s| s.origin == origin).cloned().collect();
            if subset.is_empty() {
                continue;
            }
            let (r, m) = dispatch(ctx, kind, &subset).await?;
            metrics.insert(format!("{kind:?}").to_lowercase(), m);
            differential.extend(r);
        }
        // Persist oracle-derived expectations so mutation testing can use them.
        let mut oracle_derived = 0;
        for r in &differential {
            if let Some(exp) = r.details.get("expected_output").filter(|v| !v.is_null()) {
                if let Some(mut s) = scenarios.iter().find(|s| s.id == r.scenario_id).cloned() {
                    if s.expected_output.is_none() && r.passed {
                        s.expected_output = Some(exp.clone());
                        ctx.knowledge.upsert_scenario(s).await?;
                        oracle_derived += 1;
                    }
                }
            }
        }
        all_results.extend(differential);

        // 4. Mutation (uses every scenario with an expectation)
        let refreshed = ctx.knowledge.scenarios_for(&ctx.function_id).await?;
        let (r, m) = dispatch(ctx, VerificationKind::Mutation, &refreshed).await?;
        let mutation_injected = m["injected"].as_u64().unwrap_or(0);
        let mutation_detected = m["detected"].as_u64().unwrap_or(0);
        metrics.insert("mutation".into(), m);
        all_results.extend(r);

        // 5. Fault injection
        let (r, m) = dispatch(ctx, VerificationKind::Fault, &golden).await?;
        metrics.insert("fault".into(), m);
        all_results.extend(r);

        // 6. Concurrency
        let (r, m) = dispatch(ctx, VerificationKind::Concurrency, &[]).await?;
        metrics.insert("concurrency".into(), m);
        all_results.extend(r);

        // Evidence ledger (never "done", always numbers).
        let evidence = evidence_from_results(ctx, &all_results, "verification-factory");

        // Deterministic gate — the comparator engine decides, never an LLM (policy-enforced).
        ctx.authorize(&Principal::engine("comparator"), "decide_pass_fail", &ctx.function_id.0, &ActionContext::default())?;
        let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let behaviors = ctx.knowledge.behaviors_for(&ctx.function_id).await?;
        let requirements = ctx.knowledge.requirements_for(&ctx.function_id).await?;
        let passed_rules: HashSet<String> = all_results.iter().filter(|r| r.passed && r.kind != VerificationKind::Mutation).flat_map(|r| r.rule_ids.iter().map(|x| x.0.clone())).collect();
        let failed_rules: HashSet<String> = all_results.iter().filter(|r| r.is_unexplained_failure()).flat_map(|r| r.rule_ids.iter().map(|x| x.0.clone())).collect();
        let covered_behaviors: HashSet<String> = all_results.iter().filter(|r| r.kind == VerificationKind::GoldenReplay).filter_map(|r| golden.iter().find(|g| g.id == r.scenario_id).and_then(|g| g.behavior_id.clone()).map(|b| b.0)).collect();
        let unknown_risk = rules.iter().filter(|r| r.confidence < 0.70 || (r.observed_production_cases == 0 && !passed_rules.contains(&r.id.0))).count() as u64;
        let residual = amap_uncertainty::ResidualUncertainty { total_capabilities: rules.len() as u64, high_confidence_verified: rules.iter().filter(|r| passed_rules.contains(&r.id.0) && !failed_rules.contains(&r.id.0)).count() as u64, known_unresolved: failed_rules.len() as u64, unknown_risk_candidates: unknown_risk };

        // Rows from this run only (the lake also holds history).
        let rows = rows_from_results(&all_results);
        let (cert, eq) = build_certificate(
            &rows,
            CertificateInputs {
                function_id: ctx.function_id.0.clone(),
                implemented: all_results.iter().any(|r| r.kind == VerificationKind::Static && r.passed),
                requirements_total: requirements.len() as u64,
                rules_total: rules.len() as u64,
                rules_covered: rules.iter().filter(|r| passed_rules.contains(&r.id.0)).count() as u64,
                critical_rules_total: rules.iter().filter(|r| r.priority.is_critical()).count() as u64,
                critical_rules_covered: rules.iter().filter(|r| r.priority.is_critical() && passed_rules.contains(&r.id.0)).count() as u64,
                behaviors_total: behaviors.len() as u64,
                behaviors_covered: covered_behaviors.len() as u64,
                mutation_injected,
                mutation_detected,
                residual_uncertainty: residual.ratio(),
            },
        );
        let gate = evaluate_gate(&cert, &eq, &ctx.config.thresholds);
        let failures: Vec<Value> = all_results
            .iter()
            .filter(|r| r.is_unexplained_failure() && r.kind != VerificationKind::Mutation)
            .take(25)
            .map(|r| json!({ "scenario": r.scenario_id, "kind": r.kind, "rules": r.rule_ids, "differences": r.differences.iter().take(5).collect::<Vec<_>>(), "details": r.details }))
            .collect();
        let total = all_results.iter().filter(|r| r.kind != VerificationKind::Mutation).count();
        let passed = all_results.iter().filter(|r| r.kind != VerificationKind::Mutation && r.passed).count();
        ctx.emit(if gate.certified { "verification.passed" } else { "verification.failed" }, json!({ "passed": passed, "total": total, "unexplained": cert.unexplained_differences })).await;

        let summary = format!(
            "verification: {passed}/{total} passed, mutation {mutation_detected}/{mutation_injected}, unexplained {}, residual uncertainty {:.2}% → {}",
            cert.unexplained_differences,
            residual.ratio() * 100.0,
            if gate.certified { "FE CERTIFIED" } else { "GATE FAILED" }
        );
        let mut res = AgentResult::new(
            summary,
            json!({
                "certified": gate.certified,
                "gate": gate,
                "certificate": cert,
                "equivalence": if total == 0 { 0.0 } else { passed as f64 / total as f64 },
                "equivalence_metrics": eq,
                "residual": residual,
                "failures": failures,
                "metrics": metrics,
                "oracle_derived_expectations": oracle_derived,
            }),
        );
        res.evidence = evidence;
        Ok(res)
    }
}

pub fn rows_from_results(results: &[VerificationResult]) -> Vec<amap_evidence::KindRow> {
    let mut map: std::collections::BTreeMap<(String, String), amap_evidence::KindRow> = Default::default();
    for r in results {
        let kind = serde_json::to_value(r.kind).unwrap().as_str().unwrap().to_string();
        let prio = format!("{:?}", r.priority);
        let row = map.entry((kind.clone(), prio.clone())).or_insert(amap_evidence::KindRow { kind, priority: prio, total: 0, passed: 0, unexplained: 0 });
        row.total += 1;
        if r.passed {
            row.passed += 1;
        }
        if r.is_unexplained_failure() {
            row.unexplained += 1;
        }
    }
    map.into_values().collect()
}
