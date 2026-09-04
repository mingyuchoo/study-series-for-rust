//! Verification Factory runner (design §9): runs every independent engine, writes evidence,
//! and computes the quality gate deterministically. Remote workers are used when configured.
use crate::evidence_from_results;
use crate::verification::run_engine;
use amap_domain::*;
use amap_evidence::{build_certificate, CertificateInputs};
use amap_orchestrator::proto::verification_client::VerificationClient;
use amap_orchestrator::proto::{Artifact, VerifyRequest};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_policy::{ActionContext, Principal};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;
use tonic::metadata::MetadataValue;
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity};
use tonic::Request;
use walkdir::WalkDir;

#[derive(Default)]
pub struct VerifierAgent;

fn collect_tree(
    root: &Path,
    label: &str,
    artifacts: &mut Vec<Artifact>,
    total: &mut usize,
    limit: usize,
) -> Result<(), OrchestrationError> {
    if !root.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|error| OrchestrationError::Other(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| OrchestrationError::Other(error.to_string()))?;
        if relative.components().any(|component| {
            matches!(
                component.as_os_str().to_str(),
                Some(".git" | "target" | ".amap" | ".amap-staging" | ".amap-mutants")
            )
        }) {
            continue;
        }
        let data = std::fs::read(entry.path())
            .map_err(|error| OrchestrationError::Other(error.to_string()))?;
        *total = total.saturating_add(data.len());
        if *total > limit {
            return Err(OrchestrationError::Other(format!(
                "worker artifact bundle exceeds {limit} bytes"
            )));
        }
        artifacts.push(Artifact {
            root: label.to_string(),
            path: relative.to_string_lossy().to_string(),
            original_path: entry.path().to_string_lossy().to_string(),
            data,
        });
    }
    Ok(())
}

fn collect_artifacts(ctx: &AgentContext) -> Result<Vec<Artifact>, OrchestrationError> {
    let limit = if ctx.config.verifier_artifact_max_bytes == 0 {
        64 * 1024 * 1024
    } else {
        ctx.config.verifier_artifact_max_bytes
    };
    let mut artifacts = Vec::new();
    let mut total = 0;
    collect_tree(
        &ctx.config.source_root,
        "source",
        &mut artifacts,
        &mut total,
        limit,
    )?;
    collect_tree(
        &ctx.config.workspace,
        "workspace",
        &mut artifacts,
        &mut total,
        limit,
    )?;
    let support = ctx
        .config
        .documents
        .iter()
        .chain(ctx.config.comparator_specs.iter())
        .chain(ctx.config.traces.iter())
        .chain(ctx.config.invariants.iter())
        .chain(
            ctx.config
                .trace_sources
                .iter()
                .filter_map(|source| match source {
                    amap_orchestrator::TraceSource::File { path } => Some(path),
                    amap_orchestrator::TraceSource::Kafka { .. } => None,
                }),
        );
    for (index, path) in support.enumerate() {
        if path.starts_with(&ctx.config.source_root) || path.starts_with(&ctx.config.workspace) {
            continue;
        }
        let data = std::fs::read(path)
            .map_err(|error| OrchestrationError::Other(format!("{}: {error}", path.display())))?;
        total = total.saturating_add(data.len());
        if total > limit {
            return Err(OrchestrationError::Other(format!(
                "worker artifact bundle exceeds {limit} bytes"
            )));
        }
        artifacts.push(Artifact {
            root: "support".into(),
            path: format!(
                "{index}-{}",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("file")
            ),
            original_path: path.to_string_lossy().to_string(),
            data,
        });
    }
    Ok(artifacts)
}

async fn dispatch(
    ctx: &AgentContext,
    kind: VerificationKind,
    scenarios: &[TestScenario],
) -> Result<(Vec<VerificationResult>, Value), OrchestrationError> {
    if let Some(endpoint) = &ctx.config.verifier_endpoint {
        let mut transport = Endpoint::from_shared(endpoint.clone())
            .map_err(|e| OrchestrationError::Other(format!("worker endpoint: {e}")))?;
        if let Some(ca_path) = &ctx.config.verifier_tls_ca {
            let ca = std::fs::read(ca_path)
                .map_err(|e| OrchestrationError::Other(format!("worker CA: {e}")))?;
            let mut tls = ClientTlsConfig::new().ca_certificate(Certificate::from_pem(ca));
            if let Some(domain) = &ctx.config.verifier_tls_domain {
                tls = tls.domain_name(domain);
            }
            match (&ctx.config.verifier_tls_cert, &ctx.config.verifier_tls_key) {
                (Some(cert), Some(key)) => {
                    tls = tls.identity(Identity::from_pem(
                        std::fs::read(cert).map_err(|e| {
                            OrchestrationError::Other(format!("worker client cert: {e}"))
                        })?,
                        std::fs::read(key).map_err(|e| {
                            OrchestrationError::Other(format!("worker client key: {e}"))
                        })?,
                    ));
                }
                (None, None) => {}
                _ => {
                    return Err(OrchestrationError::Other(
                        "both verifier TLS certificate and key are required".into(),
                    ));
                }
            }
            transport = transport
                .tls_config(tls)
                .map_err(|e| OrchestrationError::Other(format!("worker TLS: {e}")))?;
        } else if !ctx.config.verifier_allow_insecure {
            return Err(OrchestrationError::Other(
                "remote verification requires verifier_tls_ca (or explicit insecure development mode)"
                    .into(),
            ));
        }
        let channel = transport
            .connect()
            .await
            .map_err(|e| OrchestrationError::Other(format!("worker connect: {e}")))?;
        let message_limit = if ctx.config.verifier_artifact_max_bytes == 0 {
            64 * 1024 * 1024
        } else {
            ctx.config.verifier_artifact_max_bytes
        };
        let mut client = VerificationClient::new(channel).max_encoding_message_size(message_limit);
        let req = VerifyRequest {
            run_id: ctx.run_id.0.clone(),
            function_id: ctx.function_id.0.clone(),
            kind: serde_json::to_value(kind)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
            scenarios_json: serde_json::to_vec(scenarios).unwrap(),
            config_json: serde_json::to_vec(&*ctx.config).unwrap(),
            artifacts: collect_artifacts(ctx)?,
        };
        let mut request = Request::new(req);
        if let Some(token) = &ctx.config.verifier_token {
            let value = MetadataValue::try_from(format!("Bearer {token}"))
                .map_err(|e| OrchestrationError::Other(format!("worker token: {e}")))?;
            request.metadata_mut().insert("authorization", value);
        }
        let resp = client
            .verify(request)
            .await
            .map_err(|e| OrchestrationError::Other(format!("worker verify: {e}")))?
            .into_inner();
        let results: Vec<VerificationResult> = serde_json::from_slice(&resp.results_json)
            .map_err(|e| OrchestrationError::Other(e.to_string()))?;
        let metrics: Value = serde_json::from_slice(&resp.metrics_json).unwrap_or(json!({}));
        return Ok((results, metrics));
    }
    run_engine(kind, scenarios, &ctx.config, &ctx.function_id)
        .await
        .map_err(|e| OrchestrationError::agent("verify", e))
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
        let golden: Vec<TestScenario> = scenarios
            .iter()
            .filter(|s| s.origin == ScenarioOrigin::GoldenMaster)
            .cloned()
            .collect();
        let derived: Vec<TestScenario> = scenarios
            .iter()
            .filter(|s| s.origin != ScenarioOrigin::GoldenMaster)
            .cloned()
            .collect();
        let mut all_results: Vec<VerificationResult> = Vec::new();
        let mut metrics = serde_json::Map::new();

        // 1. Static
        let (r, m) = dispatch(ctx, VerificationKind::Static, &[]).await?;
        metrics.insert("static".into(), m);
        all_results.extend(r);

        // 2. Unit scenarios derived independently from rules, without the legacy oracle.
        let unit: Vec<TestScenario> = derived
            .iter()
            .filter(|s| s.origin == ScenarioOrigin::Property && s.expected_output.is_some())
            .cloned()
            .collect();
        if !unit.is_empty() {
            let (r, m) = dispatch(ctx, VerificationKind::Unit, &unit).await?;
            metrics.insert("unit".into(), m);
            all_results.extend(r);
        }

        // 3. Golden replay (production behaviors as oracle)
        let (r, m) = dispatch(ctx, VerificationKind::GoldenReplay, &golden).await?;
        metrics.insert("golden_replay".into(), m);
        all_results.extend(r);

        // 4. Differential comparison for manually supplied cases. Generated boundary, property
        // and adversarial cases are compared below under their own engine identities, so running
        // them here again would double-count both evidence and defects.
        let manual: Vec<TestScenario> = derived
            .iter()
            .filter(|scenario| scenario.origin == ScenarioOrigin::Manual)
            .cloned()
            .collect();
        if ctx.config.legacy_command.is_some() && !manual.is_empty() {
            let (r, m) = dispatch(ctx, VerificationKind::Differential, &manual).await?;
            metrics.insert("differential".into(), m);
            all_results.extend(r);
        }

        // 5. State and interface verification are independently reported for golden captures.
        for kind in [VerificationKind::State, VerificationKind::Interface] {
            if !golden.is_empty() {
                let (r, m) = dispatch(ctx, kind, &golden).await?;
                metrics.insert(format!("{kind:?}").to_lowercase(), m);
                all_results.extend(r);
            }
        }

        // 6. Boundary / property / adversarial engines + invariants
        let mut differential: Vec<VerificationResult> = Vec::new();
        for (origin, kind) in [
            (ScenarioOrigin::Boundary, VerificationKind::Boundary),
            (ScenarioOrigin::Property, VerificationKind::Property),
            (ScenarioOrigin::Adversarial, VerificationKind::Adversarial),
        ] {
            let subset: Vec<TestScenario> = derived
                .iter()
                .filter(|s| s.origin == origin)
                .cloned()
                .collect();
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

        // 7. Mutation (uses every scenario with an expectation)
        let refreshed = ctx.knowledge.scenarios_for(&ctx.function_id).await?;
        let (r, m) = dispatch(ctx, VerificationKind::Mutation, &refreshed).await?;
        let mutation_injected = m["injected"].as_u64().unwrap_or(0);
        let mutation_detected = m["detected"].as_u64().unwrap_or(0);
        metrics.insert("mutation".into(), m);
        all_results.extend(r);

        // 8. Fault injection
        let (r, m) = dispatch(ctx, VerificationKind::Fault, &golden).await?;
        metrics.insert("fault".into(), m);
        all_results.extend(r);

        // 9. Concurrency
        let (r, m) = dispatch(ctx, VerificationKind::Concurrency, &[]).await?;
        metrics.insert("concurrency".into(), m);
        all_results.extend(r);

        // Evidence ledger (never "done", always numbers).
        let evidence = evidence_from_results(ctx, &all_results, "verification-factory");

        // Deterministic gate — the comparator engine decides, never an LLM (policy-enforced).
        ctx.authorize(
            &Principal::engine("comparator"),
            "decide_pass_fail",
            &ctx.function_id.0,
            &ActionContext::default(),
        )?;
        let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let behaviors = ctx.knowledge.behaviors_for(&ctx.function_id).await?;
        let requirements = ctx.knowledge.requirements_for(&ctx.function_id).await?;
        let evidenced_rules: HashSet<String> = all_results
            .iter()
            .filter(|r| r.kind != VerificationKind::Mutation)
            .flat_map(|r| r.rule_ids.iter().map(|x| x.0.clone()))
            .collect();
        let failed_rules: HashSet<String> = all_results
            .iter()
            .filter(|r| r.is_unexplained_failure())
            .flat_map(|r| r.rule_ids.iter().map(|x| x.0.clone()))
            .collect();
        let verified_rules: HashSet<String> =
            evidenced_rules.difference(&failed_rules).cloned().collect();
        let covered_requirements: HashSet<String> = rules
            .iter()
            .filter(|rule| verified_rules.contains(&rule.id.0))
            .flat_map(|rule| rule.requirement_ids.iter().map(|id| id.0.clone()))
            .collect();
        let covered_behaviors: HashSet<String> = all_results
            .iter()
            .filter(|r| r.kind == VerificationKind::GoldenReplay)
            .filter_map(|r| {
                golden
                    .iter()
                    .find(|g| g.id == r.scenario_id)
                    .and_then(|g| g.behavior_id.clone())
                    .map(|b| b.0)
            })
            .collect();
        let unknown_risk = rules
            .iter()
            .filter(|r| {
                !failed_rules.contains(&r.id.0)
                    && (r.confidence < 0.70
                        || (r.observed_production_cases == 0 && !verified_rules.contains(&r.id.0)))
            })
            .count() as u64;
        let residual = amap_uncertainty::ResidualUncertainty {
            total_capabilities: rules.len() as u64,
            high_confidence_verified: rules
                .iter()
                .filter(|r| r.confidence >= 0.70 && verified_rules.contains(&r.id.0))
                .count() as u64,
            known_unresolved: failed_rules.len() as u64,
            unknown_risk_candidates: unknown_risk,
        };

        // Rows from this run only (the lake also holds history).
        let rows = rows_from_results(&all_results);
        let (cert, eq) = build_certificate(
            &rows,
            CertificateInputs {
                function_id: ctx.function_id.0.clone(),
                implemented: all_results
                    .iter()
                    .any(|r| r.kind == VerificationKind::Static && r.passed),
                requirements_total: requirements.len() as u64,
                requirements_covered: requirements
                    .iter()
                    .filter(|requirement| covered_requirements.contains(&requirement.id.0))
                    .count() as u64,
                rules_total: rules.len() as u64,
                rules_covered: rules
                    .iter()
                    .filter(|r| verified_rules.contains(&r.id.0))
                    .count() as u64,
                critical_rules_total: rules.iter().filter(|r| r.priority.is_critical()).count()
                    as u64,
                critical_rules_covered: rules
                    .iter()
                    .filter(|r| r.priority.is_critical() && verified_rules.contains(&r.id.0))
                    .count() as u64,
                behaviors_total: behaviors.len() as u64,
                behaviors_covered: covered_behaviors.len() as u64,
                mutation_injected,
                mutation_detected,
                residual_uncertainty: residual.ratio(),
            },
        );
        let mut gate = evaluate_gate(&cert, &eq, &ctx.config.thresholds);
        if gate.certified {
            let human_approved = ctx.knowledge.list_reviews().await?.iter().any(|review| {
                review.run_id.as_ref() == Some(&ctx.run_id)
                    && review.function_id == ctx.function_id
                    && review.status == amap_knowledge::ReviewStatus::Approved
            });
            let decision = ctx.policy.authorize(
                &Principal::engine("gate"),
                "certify",
                &ctx.function_id.0,
                &ActionContext {
                    is_critical: rules.iter().any(|rule| rule.priority.is_critical()),
                    independent_verifier: true,
                    human_approved,
                    ..ActionContext::default().with_uncertainty(residual.ratio())
                },
            )?;
            gate.checks.push(GateCheck {
                kpi: "Governance Policy".into(),
                required: "allow".into(),
                actual: if decision.allowed {
                    "allow".into()
                } else {
                    format!("deny: {}", decision.reasons.join(", "))
                },
                passed: decision.allowed,
            });
            gate.certified = decision.allowed;
        }
        let failures: Vec<Value> = all_results
            .iter()
            .filter(|r| r.is_unexplained_failure() && r.kind != VerificationKind::Mutation)
            .take(25)
            .map(|r| json!({ "scenario": r.scenario_id, "kind": r.kind, "rules": r.rule_ids, "differences": r.differences.iter().take(5).collect::<Vec<_>>(), "details": r.details }))
            .collect();
        let total = all_results
            .iter()
            .filter(|r| r.kind != VerificationKind::Mutation)
            .count();
        let passed = all_results
            .iter()
            .filter(|r| r.kind != VerificationKind::Mutation && r.passed)
            .count();
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
    let mut map: std::collections::BTreeMap<(String, String), amap_evidence::KindRow> =
        Default::default();
    for r in results {
        let kind = serde_json::to_value(r.kind)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        let prio = format!("{:?}", r.priority);
        let row = map
            .entry((kind.clone(), prio.clone()))
            .or_insert(amap_evidence::KindRow {
                kind,
                priority: prio,
                total: 0,
                passed: 0,
                unexplained: 0,
            });
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
