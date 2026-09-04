//! Agent governance policies evaluated with Cedar (design §10).
//!
//! Cedar decides *who can do what*. Business invariants live in `amap-invariant`.

use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityId, EntityTypeName, EntityUid, PolicySet,
    Request, RestrictedExpression,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const DEFAULT_POLICIES: &str = include_str!("agent_governance.cedar");

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("policy parse error: {0}")]
    Parse(String),
    #[error("request error: {0}")]
    Request(String),
}

/// Principal kinds. `Agent` = LLM-backed agent, `Engine` = deterministic engine, `Human` = person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrincipalKind {
    Agent,
    Engine,
    Human,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Principal {
    pub kind: PrincipalKind,
    pub id: String,
}

impl Principal {
    pub fn agent(id: &str) -> Self {
        Self {
            kind: PrincipalKind::Agent,
            id: id.into(),
        }
    }
    pub fn engine(id: &str) -> Self {
        Self {
            kind: PrincipalKind::Engine,
            id: id.into(),
        }
    }
    pub fn human(id: &str) -> Self {
        Self {
            kind: PrincipalKind::Human,
            id: id.into(),
        }
    }
    fn uid(&self) -> EntityUid {
        let ty = match self.kind {
            PrincipalKind::Agent => "Agent",
            PrincipalKind::Engine => "Engine",
            PrincipalKind::Human => "Human",
        };
        EntityUid::from_type_name_and_id(
            EntityTypeName::from_str(ty).unwrap(),
            EntityId::new(&self.id),
        )
    }
}

/// Context attributes consulted by the policies.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ActionContext {
    /// Agent that authored the artefact being acted upon.
    pub author: Option<Principal>,
    pub is_critical: bool,
    pub independent_verifier: bool,
    /// Uncertainty in basis points (0.2 = 2000).
    pub uncertainty_bp: i64,
    pub human_approved: bool,
}

impl ActionContext {
    pub fn with_uncertainty(mut self, u: f64) -> Self {
        self.uncertainty_bp = (u * 10_000.0).round() as i64;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub reasons: Vec<String>,
}

pub struct PolicyEngine {
    policies: PolicySet,
    authorizer: Authorizer,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::from_source(DEFAULT_POLICIES).expect("built-in policies parse")
    }
}

impl PolicyEngine {
    pub fn from_source(src: &str) -> Result<Self, PolicyError> {
        let policies = PolicySet::from_str(src).map_err(|e| PolicyError::Parse(e.to_string()))?;
        Ok(Self {
            policies,
            authorizer: Authorizer::new(),
        })
    }

    pub fn authorize(
        &self,
        principal: &Principal,
        action: &str,
        resource: &str,
        ctx: &ActionContext,
    ) -> Result<PolicyDecision, PolicyError> {
        let action_uid = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("Action").unwrap(),
            EntityId::new(action),
        );
        let resource_uid = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("Resource").unwrap(),
            EntityId::new(resource),
        );

        let mut pairs: Vec<(String, RestrictedExpression)> = vec![
            (
                "is_critical".into(),
                RestrictedExpression::new_bool(ctx.is_critical),
            ),
            (
                "independent_verifier".into(),
                RestrictedExpression::new_bool(ctx.independent_verifier),
            ),
            (
                "uncertainty_bp".into(),
                RestrictedExpression::new_long(ctx.uncertainty_bp),
            ),
            (
                "human_approved".into(),
                RestrictedExpression::new_bool(ctx.human_approved),
            ),
        ];
        if let Some(author) = &ctx.author {
            pairs.push((
                "author".into(),
                RestrictedExpression::new_entity_uid(author.uid()),
            ));
        }
        let context =
            Context::from_pairs(pairs).map_err(|e| PolicyError::Request(e.to_string()))?;
        let request = Request::new(principal.uid(), action_uid, resource_uid, context, None)
            .map_err(|e| PolicyError::Request(e.to_string()))?;
        let response = self
            .authorizer
            .is_authorized(&request, &self.policies, &Entities::empty());
        let reasons = response
            .diagnostics()
            .reason()
            .map(|p| p.to_string())
            .collect();
        Ok(PolicyDecision {
            allowed: response.decision() == Decision::Allow,
            reasons,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_cannot_approve_own_change() {
        let e = PolicyEngine::default();
        let ctx = ActionContext {
            author: Some(Principal::agent("builder")),
            ..Default::default()
        };
        assert!(
            !e.authorize(&Principal::agent("builder"), "approve", "patch-1", &ctx)
                .unwrap()
                .allowed
        );
        assert!(
            e.authorize(&Principal::agent("reviewer"), "approve", "patch-1", &ctx)
                .unwrap()
                .allowed
        );
    }

    #[test]
    fn llm_never_decides_pass_fail() {
        let e = PolicyEngine::default();
        let ctx = ActionContext::default();
        assert!(
            !e.authorize(
                &Principal::agent("reviewer"),
                "decide_pass_fail",
                "FN-1",
                &ctx
            )
            .unwrap()
            .allowed
        );
        assert!(
            e.authorize(
                &Principal::engine("comparator"),
                "decide_pass_fail",
                "FN-1",
                &ctx
            )
            .unwrap()
            .allowed
        );
    }

    #[test]
    fn uncertainty_requires_human() {
        let e = PolicyEngine::default();
        let ctx = ActionContext {
            independent_verifier: true,
            ..Default::default()
        }
        .with_uncertainty(0.25);
        assert!(
            !e.authorize(&Principal::engine("gate"), "certify", "FN-1", &ctx)
                .unwrap()
                .allowed
        );
        let ok = ActionContext {
            human_approved: true,
            ..ctx
        };
        assert!(
            e.authorize(&Principal::engine("gate"), "certify", "FN-1", &ok)
                .unwrap()
                .allowed
        );
    }
}
