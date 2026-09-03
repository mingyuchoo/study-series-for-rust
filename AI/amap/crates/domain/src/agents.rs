use serde::{Deserialize, Serialize};

/// Every agent role in the system. Builder and Verifier are structurally separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Orchestrator,
    Discovery,
    RuleMiner,
    BehaviorMiner,
    Uncertainty,
    Architecture,
    Builder,
    TestGenerator,
    Boundary,
    Adversarial,
    Mutation,
    Fault,
    Concurrency,
    Invariant,
    Replay,
    Rca,
    Fix,
    Reviewer,
    BusinessReviewer,
    Human,
}

impl AgentRole {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentRole::Orchestrator => "orchestrator",
            AgentRole::Discovery => "discovery",
            AgentRole::RuleMiner => "rule_miner",
            AgentRole::BehaviorMiner => "behavior_miner",
            AgentRole::Uncertainty => "uncertainty",
            AgentRole::Architecture => "architecture",
            AgentRole::Builder => "builder",
            AgentRole::TestGenerator => "test_generator",
            AgentRole::Boundary => "boundary",
            AgentRole::Adversarial => "adversarial",
            AgentRole::Mutation => "mutation",
            AgentRole::Fault => "fault",
            AgentRole::Concurrency => "concurrency",
            AgentRole::Invariant => "invariant",
            AgentRole::Replay => "replay",
            AgentRole::Rca => "rca",
            AgentRole::Fix => "fix",
            AgentRole::Reviewer => "reviewer",
            AgentRole::BusinessReviewer => "business_reviewer",
            AgentRole::Human => "human",
        }
    }
}

/// LLM providers behind the gateway. Agents never talk to a vendor directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProvider {
    Anthropic,
    OpenAi,
    Bedrock,
    Local,
    Mock,
}

/// Risk-based HITL routing tiers (design §21).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitlTier {
    /// confidence > 0.98
    Auto,
    /// 0.90 – 0.98
    AiCrossReview,
    /// 0.70 – 0.90
    ExpertReview,
    /// < 0.70
    SmeMandatory,
}

impl HitlTier {
    pub fn from_confidence(confidence: f64) -> Self {
        if confidence > 0.98 {
            HitlTier::Auto
        } else if confidence >= 0.90 {
            HitlTier::AiCrossReview
        } else if confidence >= 0.70 {
            HitlTier::ExpertReview
        } else {
            HitlTier::SmeMandatory
        }
    }
    pub fn requires_human(self) -> bool {
        matches!(self, HitlTier::ExpertReview | HitlTier::SmeMandatory)
    }
}
