//! Uncertainty Engine (design §15, §24).
//!
//! Every score here is derived from *evidence*, never from an LLM's self-reported confidence.
//! `U = f(R, B, T, D, E, C)` where R = requirement confidence, B = behavior coverage,
//! T = test coverage, D = dependency uncertainty, E = equivalence evidence, C = complexity.

use amap_domain::{EvidenceSources, HitlTier};
use serde::{Deserialize, Serialize};

/// Confidence components for a business function (all in `0.0..=1.0`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceVector {
    pub requirement: f64,
    pub rule: f64,
    pub behavior: f64,
    pub test: f64,
    /// 1.0 = every dependency is itself high-confidence.
    pub dependency: f64,
    pub equivalence: f64,
    /// Normalised complexity in `0..=1` (higher = more complex = more uncertain).
    pub complexity: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceEvidence {
    pub requirement_confidences: Vec<f64>,
    pub rule_confidences: Vec<f64>,
    pub rules_total: usize,
    pub rules_observed_in_production: usize,
    pub scenarios_total: usize,
    pub source_units_total: usize,
    pub unresolved_dependencies: usize,
    pub equivalence: f64,
}

pub fn confidence_vector_from_evidence(evidence: &ConfidenceEvidence) -> ConfidenceVector {
    let average = |values: &[f64]| {
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        }
    };
    let requirement = if evidence.requirement_confidences.is_empty() {
        0.5
    } else {
        average(&evidence.requirement_confidences)
    };
    let behavior = if evidence.rules_total == 0 {
        0.0
    } else {
        evidence.rules_observed_in_production as f64 / evidence.rules_total as f64
    };
    let test = if evidence.rules_total == 0 {
        0.0
    } else {
        (evidence.scenarios_total as f64 / (evidence.rules_total as f64 * 3.0)).min(1.0)
    };
    let dependency = if evidence.source_units_total == 0 {
        1.0
    } else {
        1.0 - (evidence.unresolved_dependencies as f64 / evidence.source_units_total as f64)
            .min(0.5)
    };

    ConfidenceVector {
        requirement,
        rule: average(&evidence.rule_confidences),
        behavior,
        test,
        dependency,
        equivalence: evidence.equivalence,
        complexity: (evidence.source_units_total as f64 / 200.0).min(1.0),
    }
}

impl Default for ConfidenceVector {
    fn default() -> Self {
        Self {
            requirement: 0.5,
            rule: 0.5,
            behavior: 0.0,
            test: 0.0,
            dependency: 1.0,
            equivalence: 0.0,
            complexity: 0.5,
        }
    }
}

/// Weights for the components. Sum of positive weights = 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weights {
    pub requirement: f64,
    pub rule: f64,
    pub behavior: f64,
    pub test: f64,
    pub dependency: f64,
    pub equivalence: f64,
    /// Penalty weight applied to complexity.
    pub complexity: f64,
}

impl Default for Weights {
    fn default() -> Self {
        // Production behaviour and equivalence evidence dominate (design §29 ranking).
        Self {
            requirement: 0.10,
            rule: 0.15,
            behavior: 0.25,
            test: 0.15,
            dependency: 0.10,
            equivalence: 0.25,
            complexity: 0.10,
        }
    }
}

/// Evidence-based adjustments to a rule's confidence (design §15 table).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EvidenceSignals {
    pub production_evidence_exists: bool,
    pub source_and_production_agree: bool,
    pub two_models_agree: bool,
    pub models_disagree: bool,
    pub boundary_tests_passed: bool,
    pub mutation_survived: bool,
    pub unexplained_diff_exists: bool,
    pub human_confirmed: bool,
}

impl EvidenceSignals {
    pub fn adjustment(&self) -> f64 {
        let mut a = 0.0;
        if self.production_evidence_exists {
            a += 0.25;
        }
        if self.source_and_production_agree {
            a += 0.20;
        }
        if self.two_models_agree {
            a += 0.05;
        }
        if self.models_disagree {
            a -= 0.20;
        }
        if self.boundary_tests_passed {
            a += 0.10;
        }
        if self.mutation_survived {
            a -= 0.30;
        }
        if self.unexplained_diff_exists {
            a -= 0.50;
        }
        if self.human_confirmed {
            a += 0.15;
        }
        a
    }
}

pub fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Deterministic rule confidence = baseline(evidence sources) + evidence adjustments, clamped.
/// Baseline is scaled so that adjustments refine rather than dominate.
pub fn rule_confidence(sources: &EvidenceSources, signals: &EvidenceSignals) -> f64 {
    let base = sources.baseline_confidence();
    // Adjustments move the residual gap (1-base) or the base itself proportionally.
    let adj = signals.adjustment();
    let value = if adj >= 0.0 {
        base + (1.0 - base) * adj.min(1.0)
    } else {
        base * (1.0 + adj).max(0.0)
    };
    (clamp01(value) * 10_000.0).round() / 10_000.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyReport {
    pub confidence: f64,
    pub uncertainty: f64,
    pub tier: HitlTier,
    pub vector: ConfidenceVector,
    pub drivers: Vec<(String, f64)>,
}

/// Compute overall uncertainty for a function.
pub fn assess(v: &ConfidenceVector, w: &Weights) -> UncertaintyReport {
    let weighted = w.requirement * clamp01(v.requirement)
        + w.rule * clamp01(v.rule)
        + w.behavior * clamp01(v.behavior)
        + w.test * clamp01(v.test)
        + w.dependency * clamp01(v.dependency)
        + w.equivalence * clamp01(v.equivalence);
    let confidence = clamp01(weighted * (1.0 - w.complexity * clamp01(v.complexity)));
    let uncertainty = 1.0 - confidence;
    let mut drivers = vec![
        (
            "requirement".to_string(),
            w.requirement * (1.0 - clamp01(v.requirement)),
        ),
        ("rule".to_string(), w.rule * (1.0 - clamp01(v.rule))),
        (
            "behavior".to_string(),
            w.behavior * (1.0 - clamp01(v.behavior)),
        ),
        ("test".to_string(), w.test * (1.0 - clamp01(v.test))),
        (
            "dependency".to_string(),
            w.dependency * (1.0 - clamp01(v.dependency)),
        ),
        (
            "equivalence".to_string(),
            w.equivalence * (1.0 - clamp01(v.equivalence)),
        ),
        (
            "complexity".to_string(),
            w.complexity * clamp01(v.complexity),
        ),
    ];
    drivers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    UncertaintyReport {
        confidence,
        uncertainty,
        tier: HitlTier::from_confidence(confidence),
        vector: *v,
        drivers,
    }
}

/// Residual uncertainty across all capabilities (the single most important KPI).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResidualUncertainty {
    pub total_capabilities: u64,
    pub high_confidence_verified: u64,
    pub known_unresolved: u64,
    pub unknown_risk_candidates: u64,
}

impl ResidualUncertainty {
    pub fn ratio(&self) -> f64 {
        if self.total_capabilities == 0 {
            return 1.0;
        }
        self.known_unresolved
            .saturating_add(self.unknown_risk_candidates)
            .min(self.total_capabilities) as f64
            / self.total_capabilities as f64
    }
}

/// N-version reasoning: agreement between independent model answers raises confidence, disagreement is a risk signal.
pub fn n_version_agreement(answers: &[String]) -> (bool, f64) {
    if answers.len() < 2 {
        return (false, 0.0);
    }
    let norm: Vec<String> = answers
        .iter()
        .map(|a| {
            a.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        })
        .collect();
    let first = &norm[0];
    let agree = norm.iter().filter(|a| *a == first).count();
    let ratio = agree as f64 / norm.len() as f64;
    (ratio == 1.0, ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_follows_evidence_not_llm() {
        let code_only = EvidenceSources {
            code: true,
            ..Default::default()
        };
        let full = EvidenceSources {
            code: true,
            document: true,
            production: true,
            inferred: false,
        };
        assert!(
            rule_confidence(&full, &EvidenceSignals::default())
                > rule_confidence(&code_only, &EvidenceSignals::default())
        );
        let bad = EvidenceSignals {
            unexplained_diff_exists: true,
            ..Default::default()
        };
        assert!(rule_confidence(&full, &bad) < 0.6);
    }

    #[test]
    fn confidence_vector_is_derived_only_from_supplied_evidence() {
        let vector = confidence_vector_from_evidence(&ConfidenceEvidence {
            requirement_confidences: vec![0.8, 1.0],
            rule_confidences: vec![0.7, 0.9],
            rules_total: 2,
            rules_observed_in_production: 1,
            scenarios_total: 3,
            source_units_total: 10,
            unresolved_dependencies: 2,
            equivalence: 0.95,
        });
        assert_eq!(vector.requirement, 0.9);
        assert_eq!(vector.rule, 0.8);
        assert_eq!(vector.behavior, 0.5);
        assert_eq!(vector.test, 0.5);
        assert_eq!(vector.dependency, 0.8);
        assert_eq!(vector.equivalence, 0.95);
    }

    #[test]
    fn tiers() {
        let hi = ConfidenceVector {
            requirement: 1.0,
            rule: 1.0,
            behavior: 1.0,
            test: 1.0,
            dependency: 1.0,
            equivalence: 1.0,
            complexity: 0.0,
        };
        assert_eq!(assess(&hi, &Weights::default()).tier, HitlTier::Auto);
        let lo = ConfidenceVector {
            behavior: 0.0,
            equivalence: 0.0,
            ..hi
        };
        assert!(assess(&lo, &Weights::default()).tier.requires_human());
    }

    #[test]
    fn residual_uncertainty_is_bounded() {
        let residual = ResidualUncertainty {
            total_capabilities: 10,
            high_confidence_verified: 0,
            known_unresolved: 10,
            unknown_risk_candidates: 5,
        };
        assert_eq!(residual.ratio(), 1.0);
    }
}
