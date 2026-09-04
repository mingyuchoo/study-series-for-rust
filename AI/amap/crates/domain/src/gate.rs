use crate::evidence::FunctionCertificate;
use serde::{Deserialize, Serialize};

/// Final quality gate thresholds (design §23). All must hold; Unexplained Difference = 0 is absolute.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QualityGateThresholds {
    pub requirements_coverage: f64,
    pub critical_rule_coverage: f64,
    pub p0_functional_equivalence: f64,
    pub p1_functional_equivalence: f64,
    pub overall_functional_equivalence: f64,
    pub production_behavior_coverage: f64,
    pub business_rule_coverage: f64,
    pub mutation_detection: f64,
    pub max_unexplained_differences: u64,
    pub max_open_p0_p1_defects: u64,
    pub max_residual_uncertainty: f64,
}

impl Default for QualityGateThresholds {
    fn default() -> Self {
        Self {
            requirements_coverage: 1.0,
            critical_rule_coverage: 1.0,
            p0_functional_equivalence: 1.0,
            p1_functional_equivalence: 0.99999,
            overall_functional_equivalence: 0.999,
            production_behavior_coverage: 0.999,
            business_rule_coverage: 0.999,
            mutation_detection: 0.99,
            max_unexplained_differences: 0,
            max_open_p0_p1_defects: 0,
            max_residual_uncertainty: 0.001,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GateCheck {
    pub kpi: String,
    pub required: String,
    pub actual: String,
    pub passed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GateReport {
    pub function_id: String,
    pub certified: bool,
    pub checks: Vec<GateCheck>,
}

/// Per-priority functional equivalence measured from evidence.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EquivalenceMetrics {
    pub p0_total: u64,
    pub p0_passed: u64,
    pub p1_total: u64,
    pub p1_passed: u64,
    pub all_total: u64,
    pub all_passed: u64,
    pub behaviors_total: u64,
    pub behaviors_covered: u64,
}

fn ratio(n: u64, d: u64) -> f64 {
    if d == 0 {
        0.0
    } else {
        n as f64 / d as f64
    }
}

/// Deterministic gate evaluation — this is program logic, never an LLM verdict.
pub fn evaluate_gate(
    cert: &FunctionCertificate,
    eq: &EquivalenceMetrics,
    t: &QualityGateThresholds,
) -> GateReport {
    let mut checks = Vec::new();
    let mut push = |kpi: &str, required: String, actual: String, passed: bool| {
        checks.push(GateCheck {
            kpi: kpi.to_string(),
            required,
            actual,
            passed,
        });
    };
    let pct = |v: f64| format!("{:.4}%", v * 100.0);

    let requirements = ratio(cert.requirements_covered, cert.requirements_total);
    push(
        "Requirement Coverage",
        pct(t.requirements_coverage),
        pct(requirements),
        requirements >= t.requirements_coverage,
    );

    push(
        "Implementation Present",
        "true".into(),
        cert.implemented.to_string(),
        cert.implemented,
    );

    let crit = ratio(cert.critical_rules_covered, cert.critical_rules_total);
    push(
        "Critical Business Rule Coverage",
        pct(t.critical_rule_coverage),
        pct(crit),
        crit >= t.critical_rule_coverage,
    );

    let p0 = ratio(eq.p0_passed, eq.p0_total);
    push(
        "P0 Functional Equivalence",
        pct(t.p0_functional_equivalence),
        pct(p0),
        p0 >= t.p0_functional_equivalence,
    );

    let p1 = ratio(eq.p1_passed, eq.p1_total);
    push(
        "P1 Functional Equivalence",
        pct(t.p1_functional_equivalence),
        pct(p1),
        p1 >= t.p1_functional_equivalence,
    );

    let all = ratio(eq.all_passed, eq.all_total);
    push(
        "Functional Equivalence",
        pct(t.overall_functional_equivalence),
        pct(all),
        all >= t.overall_functional_equivalence,
    );

    let bcov = ratio(eq.behaviors_covered, eq.behaviors_total);
    push(
        "Production Behavior Coverage",
        pct(t.production_behavior_coverage),
        pct(bcov),
        bcov >= t.production_behavior_coverage,
    );

    let rcov = ratio(cert.rules_covered, cert.rules_total);
    push(
        "Business Rule Coverage",
        pct(t.business_rule_coverage),
        pct(rcov),
        rcov >= t.business_rule_coverage,
    );

    let ms = cert.mutation_score();
    push(
        "Mutation Detection",
        pct(t.mutation_detection),
        pct(ms),
        ms >= t.mutation_detection,
    );

    push(
        "Unexplained Difference",
        t.max_unexplained_differences.to_string(),
        cert.unexplained_differences.to_string(),
        cert.unexplained_differences <= t.max_unexplained_differences,
    );

    let defects = cert.p0_defects_open + cert.p1_defects_open;
    push(
        "P0/P1 unresolved defect",
        t.max_open_p0_p1_defects.to_string(),
        defects.to_string(),
        defects <= t.max_open_p0_p1_defects,
    );

    push(
        "Residual Uncertainty",
        format!("≤ {}", pct(t.max_residual_uncertainty)),
        pct(cert.residual_uncertainty),
        cert.residual_uncertainty <= t.max_residual_uncertainty,
    );

    let certified = cert.implemented && checks.iter().all(|c| c.passed);
    GateReport {
        function_id: cert.function_id.clone(),
        certified,
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_blocks_on_single_unexplained_difference() {
        let cert = FunctionCertificate {
            function_id: "FN-1".into(),
            implemented: true,
            requirements_total: 1,
            requirements_covered: 1,
            rules_total: 10,
            rules_covered: 10,
            critical_rules_total: 3,
            critical_rules_covered: 3,
            mutation_injected: 100,
            mutation_detected: 100,
            unexplained_differences: 1,
            ..Default::default()
        };
        let eq = EquivalenceMetrics {
            all_total: 10,
            all_passed: 10,
            p0_total: 1,
            p0_passed: 1,
            p1_total: 1,
            p1_passed: 1,
            behaviors_total: 1,
            behaviors_covered: 1,
        };
        let report = evaluate_gate(&cert, &eq, &QualityGateThresholds::default());
        assert!(!report.certified);
        assert!(report
            .checks
            .iter()
            .any(|c| c.kpi == "Unexplained Difference" && !c.passed));
    }

    #[test]
    fn gate_fails_closed_when_evidence_is_missing() {
        let cert = FunctionCertificate {
            function_id: "FN-EMPTY".into(),
            implemented: true,
            ..Default::default()
        };
        let report = evaluate_gate(
            &cert,
            &EquivalenceMetrics::default(),
            &QualityGateThresholds::default(),
        );
        assert!(!report.certified);
        assert!(report.checks.iter().any(|c| !c.passed));
    }
}
