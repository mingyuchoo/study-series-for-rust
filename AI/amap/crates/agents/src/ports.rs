//! Capability-scoped knowledge ports used by individual application use cases.

use amap_domain::*;
use amap_knowledge::{KResult, KnowledgeSnapshot, KnowledgeStore, ReviewRequest};
use async_trait::async_trait;

#[async_trait]
pub trait RuleMiningKnowledge: Send + Sync {
    async fn rules_for(&self, function: &FunctionId) -> KResult<Vec<BusinessRule>>;
    async fn requirements_for(&self, function: &FunctionId) -> KResult<Vec<Requirement>>;
    async fn upsert_rule(&self, rule: BusinessRule) -> KResult<()>;
    async fn add_relationship(&self, relationship: Relationship) -> KResult<()>;
}

#[async_trait]
impl<T: KnowledgeStore + ?Sized> RuleMiningKnowledge for T {
    async fn rules_for(&self, function: &FunctionId) -> KResult<Vec<BusinessRule>> {
        KnowledgeStore::rules_for(self, function).await
    }
    async fn requirements_for(&self, function: &FunctionId) -> KResult<Vec<Requirement>> {
        KnowledgeStore::requirements_for(self, function).await
    }
    async fn upsert_rule(&self, rule: BusinessRule) -> KResult<()> {
        KnowledgeStore::upsert_rule(self, rule).await
    }
    async fn add_relationship(&self, relationship: Relationship) -> KResult<()> {
        KnowledgeStore::add_relationship(self, relationship).await
    }
}

#[async_trait]
pub trait UncertaintyKnowledge: Send + Sync {
    async fn requirements_for(&self, function: &FunctionId) -> KResult<Vec<Requirement>>;
    async fn rules_for(&self, function: &FunctionId) -> KResult<Vec<BusinessRule>>;
    async fn scenarios_for(&self, function: &FunctionId) -> KResult<Vec<TestScenario>>;
    async fn source_units_for(&self, function: &FunctionId) -> KResult<Vec<CodeEntity>>;
    async fn snapshot(&self) -> KResult<KnowledgeSnapshot>;
    async fn list_reviews(&self) -> KResult<Vec<ReviewRequest>>;
    async fn queue_review(&self, review: ReviewRequest) -> KResult<()>;
}

#[async_trait]
impl<T: KnowledgeStore + ?Sized> UncertaintyKnowledge for T {
    async fn requirements_for(&self, function: &FunctionId) -> KResult<Vec<Requirement>> {
        KnowledgeStore::requirements_for(self, function).await
    }
    async fn rules_for(&self, function: &FunctionId) -> KResult<Vec<BusinessRule>> {
        KnowledgeStore::rules_for(self, function).await
    }
    async fn scenarios_for(&self, function: &FunctionId) -> KResult<Vec<TestScenario>> {
        KnowledgeStore::scenarios_for(self, function).await
    }
    async fn source_units_for(&self, function: &FunctionId) -> KResult<Vec<CodeEntity>> {
        KnowledgeStore::source_units_for(self, function).await
    }
    async fn snapshot(&self) -> KResult<KnowledgeSnapshot> {
        KnowledgeStore::snapshot(self).await
    }
    async fn list_reviews(&self) -> KResult<Vec<ReviewRequest>> {
        KnowledgeStore::list_reviews(self).await
    }
    async fn queue_review(&self, review: ReviewRequest) -> KResult<()> {
        KnowledgeStore::queue_review(self, review).await
    }
}

#[async_trait]
pub trait VerificationKnowledge: Send + Sync {
    async fn scenarios_for(&self, function: &FunctionId) -> KResult<Vec<TestScenario>>;
    async fn upsert_scenario(&self, scenario: TestScenario) -> KResult<()>;
    async fn rules_for(&self, function: &FunctionId) -> KResult<Vec<BusinessRule>>;
    async fn behaviors_for(&self, function: &FunctionId) -> KResult<Vec<BehaviorRecord>>;
    async fn requirements_for(&self, function: &FunctionId) -> KResult<Vec<Requirement>>;
    async fn list_reviews(&self) -> KResult<Vec<ReviewRequest>>;
}

#[async_trait]
impl<T: KnowledgeStore + ?Sized> VerificationKnowledge for T {
    async fn scenarios_for(&self, function: &FunctionId) -> KResult<Vec<TestScenario>> {
        KnowledgeStore::scenarios_for(self, function).await
    }
    async fn upsert_scenario(&self, scenario: TestScenario) -> KResult<()> {
        KnowledgeStore::upsert_scenario(self, scenario).await
    }
    async fn rules_for(&self, function: &FunctionId) -> KResult<Vec<BusinessRule>> {
        KnowledgeStore::rules_for(self, function).await
    }
    async fn behaviors_for(&self, function: &FunctionId) -> KResult<Vec<BehaviorRecord>> {
        KnowledgeStore::behaviors_for(self, function).await
    }
    async fn requirements_for(&self, function: &FunctionId) -> KResult<Vec<Requirement>> {
        KnowledgeStore::requirements_for(self, function).await
    }
    async fn list_reviews(&self) -> KResult<Vec<ReviewRequest>> {
        KnowledgeStore::list_reviews(self).await
    }
}

pub struct RuleMiningDependencies<'a, K: RuleMiningKnowledge + ?Sized> {
    pub knowledge: &'a K,
}

pub struct UncertaintyDependencies<'a, K: UncertaintyKnowledge + ?Sized> {
    pub knowledge: &'a K,
}

pub struct VerificationDependencies<'a, K: VerificationKnowledge + ?Sized> {
    pub knowledge: &'a K,
}
