//! Pure assurance decisions derived from verification facts.
//!
//! This crate deliberately has no filesystem, network, clock, random-number, or async runtime
//! dependency. Adapters collect facts; the functions here turn those facts into auditable results.

use amap_domain::*;
use amap_uncertainty::ResidualUncertainty;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KindRow {
    pub kind: String,
    pub priority: String,
    pub total: u64,
    pub passed: u64,
    pub unexplained: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerificationFailure {
    #[serde(rename = "scenario")]
    pub scenario_id: ScenarioId,
    pub kind: VerificationKind,
    #[serde(rename = "rules")]
    pub rule_ids: Vec<RuleId>,
    pub differences: Vec<Difference>,
    pub details: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerificationAssessment {
    pub certificate: FunctionCertificate,
    pub equivalence_metrics: EquivalenceMetrics,
    pub residual: ResidualUncertainty,
    pub gate: GateReport,
    pub failures: Vec<VerificationFailure>,
    pub total: usize,
    pub passed: usize,
}

pub struct VerificationFacts<'a> {
    pub function_id: &'a FunctionId,
    pub results: &'a [VerificationResult],
    pub rules: &'a [BusinessRule],
    pub behaviors: &'a [BehaviorRecord],
    pub requirements: &'a [Requirement],
    pub golden_scenarios: &'a [TestScenario],
    pub mutation_injected: u64,
    pub mutation_detected: u64,
    pub thresholds: &'a QualityGateThresholds,
}

pub fn rows_from_results(results: &[VerificationResult]) -> Vec<KindRow> {
    let mut rows: BTreeMap<(String, String), KindRow> = BTreeMap::new();
    for result in results {
        let kind = serde_json::to_value(result.kind)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        let priority = format!("{:?}", result.priority);
        let row = rows
            .entry((kind.clone(), priority.clone()))
            .or_insert(KindRow {
                kind,
                priority,
                total: 0,
                passed: 0,
                unexplained: 0,
            });
        row.total += 1;
        if result.passed {
            row.passed += 1;
        }
        if result.is_unexplained_failure() {
            row.unexplained += 1;
        }
    }
    rows.into_values().collect()
}

pub fn build_certificate(
    rows: &[KindRow],
    inputs: CertificateInputs,
) -> (FunctionCertificate, EquivalenceMetrics) {
    let mut certificate = FunctionCertificate {
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
    let mut equivalence = EquivalenceMetrics {
        behaviors_total: inputs.behaviors_total,
        behaviors_covered: inputs.behaviors_covered,
        ..Default::default()
    };

    for row in rows {
        let target = match row.kind.as_str() {
            "golden_replay" => Some(&mut certificate.golden),
            "boundary" => Some(&mut certificate.boundary),
            "property" | "unit" => Some(&mut certificate.property),
            "adversarial" => Some(&mut certificate.adversarial),
            "fault" => Some(&mut certificate.fault),
            "concurrency" => Some(&mut certificate.concurrency),
            "differential" | "state" | "interface" => Some(&mut certificate.production_replay),
            _ => None,
        };
        if let Some(summary) = target {
            summary.total += row.total;
            summary.passed += row.passed;
            summary.unexplained_failures += row.unexplained;
        }
        if row.kind != "mutation" {
            certificate.unexplained_differences += row.unexplained;
        }
        if matches!(row.kind.as_str(), "golden_replay" | "differential" | "unit") {
            equivalence.all_total += row.total;
            equivalence.all_passed += row.passed;
            match row.priority.as_str() {
                "P0" => {
                    equivalence.p0_total += row.total;
                    equivalence.p0_passed += row.passed;
                }
                "P1" => {
                    equivalence.p1_total += row.total;
                    equivalence.p1_passed += row.passed;
                }
                _ => {}
            }
            if row.unexplained > 0 && row.priority == "P0" {
                certificate.p0_defects_open += row.unexplained;
            }
            if row.unexplained > 0 && row.priority == "P1" {
                certificate.p1_defects_open += row.unexplained;
            }
        }
    }
    (certificate, equivalence)
}

pub fn assess_verification(facts: VerificationFacts<'_>) -> VerificationAssessment {
    let evidenced_rules: HashSet<String> = facts
        .results
        .iter()
        .filter(|result| result.kind != VerificationKind::Mutation)
        .flat_map(|result| result.rule_ids.iter().map(|id| id.0.clone()))
        .collect();
    let failed_rules: HashSet<String> = facts
        .results
        .iter()
        .filter(|result| result.is_unexplained_failure())
        .flat_map(|result| result.rule_ids.iter().map(|id| id.0.clone()))
        .collect();
    let verified_rules: HashSet<String> =
        evidenced_rules.difference(&failed_rules).cloned().collect();
    let covered_requirements: HashSet<String> = facts
        .rules
        .iter()
        .filter(|rule| verified_rules.contains(&rule.id.0))
        .flat_map(|rule| rule.requirement_ids.iter().map(|id| id.0.clone()))
        .collect();
    let covered_behaviors: HashSet<String> = facts
        .results
        .iter()
        .filter(|result| result.kind == VerificationKind::GoldenReplay)
        .filter_map(|result| {
            facts
                .golden_scenarios
                .iter()
                .find(|scenario| scenario.id == result.scenario_id)
                .and_then(|scenario| scenario.behavior_id.clone())
                .map(|id| id.0)
        })
        .collect();
    let unknown_risk_candidates = facts
        .rules
        .iter()
        .filter(|rule| {
            !failed_rules.contains(&rule.id.0)
                && (rule.confidence < 0.70
                    || (rule.observed_production_cases == 0
                        && !verified_rules.contains(&rule.id.0)))
        })
        .count() as u64;
    let residual = ResidualUncertainty {
        total_capabilities: facts.rules.len() as u64,
        high_confidence_verified: facts
            .rules
            .iter()
            .filter(|rule| rule.confidence >= 0.70 && verified_rules.contains(&rule.id.0))
            .count() as u64,
        known_unresolved: failed_rules.len() as u64,
        unknown_risk_candidates,
    };

    let rows = rows_from_results(facts.results);
    let (certificate, equivalence_metrics) = build_certificate(
        &rows,
        CertificateInputs {
            function_id: facts.function_id.0.clone(),
            implemented: facts
                .results
                .iter()
                .any(|result| result.kind == VerificationKind::Static && result.passed),
            requirements_total: facts.requirements.len() as u64,
            requirements_covered: facts
                .requirements
                .iter()
                .filter(|requirement| covered_requirements.contains(&requirement.id.0))
                .count() as u64,
            rules_total: facts.rules.len() as u64,
            rules_covered: facts
                .rules
                .iter()
                .filter(|rule| verified_rules.contains(&rule.id.0))
                .count() as u64,
            critical_rules_total: facts
                .rules
                .iter()
                .filter(|rule| rule.priority.is_critical())
                .count() as u64,
            critical_rules_covered: facts
                .rules
                .iter()
                .filter(|rule| rule.priority.is_critical() && verified_rules.contains(&rule.id.0))
                .count() as u64,
            behaviors_total: facts.behaviors.len() as u64,
            behaviors_covered: covered_behaviors.len() as u64,
            mutation_injected: facts.mutation_injected,
            mutation_detected: facts.mutation_detected,
            residual_uncertainty: residual.ratio(),
        },
    );
    let gate = evaluate_gate(&certificate, &equivalence_metrics, facts.thresholds);
    let failures = facts
        .results
        .iter()
        .filter(|result| {
            result.is_unexplained_failure() && result.kind != VerificationKind::Mutation
        })
        .take(25)
        .map(|result| VerificationFailure {
            scenario_id: result.scenario_id.clone(),
            kind: result.kind,
            rule_ids: result.rule_ids.clone(),
            differences: result.differences.iter().take(5).cloned().collect(),
            details: result.details.clone(),
        })
        .collect();
    let total = facts
        .results
        .iter()
        .filter(|result| result.kind != VerificationKind::Mutation)
        .count();
    let passed = facts
        .results
        .iter()
        .filter(|result| result.kind != VerificationKind::Mutation && result.passed)
        .count();

    VerificationAssessment {
        certificate,
        equivalence_metrics,
        residual,
        gate,
        failures,
        total,
        passed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(kind: VerificationKind, priority: Priority, passed: bool) -> VerificationResult {
        VerificationResult {
            kind,
            scenario_id: ScenarioId::new("TEST-1"),
            function_id: FunctionId::new("FN-1"),
            rule_ids: vec![RuleId::new("BR-1")],
            priority,
            passed,
            differences: vec![],
            explained: false,
            explanation: None,
            duration_ms: 0,
            details: Value::Null,
        }
    }

    #[test]
    fn rows_are_grouped_by_kind_and_priority() {
        let rows = rows_from_results(&[
            result(VerificationKind::GoldenReplay, Priority::P0, true),
            result(VerificationKind::GoldenReplay, Priority::P0, false),
            result(VerificationKind::Mutation, Priority::P1, false),
        ]);
        assert_eq!(rows.len(), 2);
        let golden = rows.iter().find(|row| row.kind == "golden_replay").unwrap();
        assert_eq!((golden.total, golden.passed, golden.unexplained), (2, 1, 1));
    }

    #[test]
    fn certificate_excludes_mutations_from_unexplained_differences() {
        let rows = rows_from_results(&[
            result(VerificationKind::GoldenReplay, Priority::P0, false),
            result(VerificationKind::Mutation, Priority::P0, false),
        ]);
        let (certificate, _) = build_certificate(
            &rows,
            CertificateInputs {
                function_id: "FN-1".into(),
                implemented: true,
                requirements_total: 0,
                requirements_covered: 0,
                rules_total: 0,
                rules_covered: 0,
                critical_rules_total: 0,
                critical_rules_covered: 0,
                behaviors_total: 0,
                behaviors_covered: 0,
                mutation_injected: 1,
                mutation_detected: 0,
                residual_uncertainty: 0.0,
            },
        );
        assert_eq!(certificate.unexplained_differences, 1);
        assert_eq!(certificate.p0_defects_open, 1);
    }
}
