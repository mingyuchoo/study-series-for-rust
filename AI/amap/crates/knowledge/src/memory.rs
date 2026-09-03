use crate::*;
use std::collections::BTreeMap;
use std::sync::RwLock;

/// Thread-safe in-memory knowledge store (tests, CLI demo, ephemeral runs).
#[derive(Default)]
pub struct InMemoryKnowledgeStore {
    inner: RwLock<KnowledgeSnapshot>,
    reviews: RwLock<BTreeMap<String, ReviewRequest>>,
}

impl InMemoryKnowledgeStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn from_snapshot(s: KnowledgeSnapshot) -> Self {
        Self { inner: RwLock::new(s), reviews: RwLock::new(BTreeMap::new()) }
    }
}

fn upsert_by<T: Clone>(v: &mut Vec<T>, item: T, same: impl Fn(&T, &T) -> bool) {
    if let Some(slot) = v.iter_mut().find(|x| same(x, &item)) {
        *slot = item;
    } else {
        v.push(item);
    }
}

#[async_trait]
impl KnowledgeStore for InMemoryKnowledgeStore {
    async fn upsert_function(&self, f: BusinessFunction) -> KResult<()> {
        upsert_by(&mut self.inner.write().unwrap().functions, f, |a, b| a.id == b.id);
        Ok(())
    }
    async fn get_function(&self, id: &FunctionId) -> KResult<Option<BusinessFunction>> {
        Ok(self.inner.read().unwrap().functions.iter().find(|f| &f.id == id).cloned())
    }
    async fn list_functions(&self) -> KResult<Vec<BusinessFunction>> {
        Ok(self.inner.read().unwrap().functions.clone())
    }
    async fn upsert_requirement(&self, r: Requirement) -> KResult<()> {
        upsert_by(&mut self.inner.write().unwrap().requirements, r, |a, b| a.id == b.id);
        Ok(())
    }
    async fn requirements_for(&self, f: &FunctionId) -> KResult<Vec<Requirement>> {
        Ok(self.inner.read().unwrap().requirements.iter().filter(|r| &r.function_id == f).cloned().collect())
    }
    async fn upsert_rule(&self, r: BusinessRule) -> KResult<()> {
        upsert_by(&mut self.inner.write().unwrap().rules, r, |a, b| a.id == b.id);
        Ok(())
    }
    async fn rules_for(&self, f: &FunctionId) -> KResult<Vec<BusinessRule>> {
        Ok(self.inner.read().unwrap().rules.iter().filter(|r| &r.function_id == f).cloned().collect())
    }
    async fn upsert_source_unit(&self, e: CodeEntity) -> KResult<()> {
        upsert_by(&mut self.inner.write().unwrap().source_units, e, |a, b| a.id == b.id);
        Ok(())
    }
    async fn source_units_for(&self, f: &FunctionId) -> KResult<Vec<CodeEntity>> {
        Ok(self.inner.read().unwrap().source_units.iter().filter(|e| e.function_id.as_ref() == Some(f)).cloned().collect())
    }
    async fn list_source_units(&self) -> KResult<Vec<CodeEntity>> {
        Ok(self.inner.read().unwrap().source_units.clone())
    }
    async fn upsert_db_entity(&self, e: DbEntity) -> KResult<()> {
        upsert_by(&mut self.inner.write().unwrap().db_entities, e, |a, b| a.id == b.id);
        Ok(())
    }
    async fn list_db_entities(&self) -> KResult<Vec<DbEntity>> {
        Ok(self.inner.read().unwrap().db_entities.clone())
    }
    async fn upsert_interface(&self, i: InterfaceSpec) -> KResult<()> {
        upsert_by(&mut self.inner.write().unwrap().interfaces, i, |a, b| a.id == b.id);
        Ok(())
    }
    async fn list_interfaces(&self) -> KResult<Vec<InterfaceSpec>> {
        Ok(self.inner.read().unwrap().interfaces.clone())
    }
    async fn upsert_behavior(&self, b: BehaviorRecord) -> KResult<()> {
        upsert_by(&mut self.inner.write().unwrap().behaviors, b, |a, b| a.id == b.id);
        Ok(())
    }
    async fn behaviors_for(&self, f: &FunctionId) -> KResult<Vec<BehaviorRecord>> {
        Ok(self.inner.read().unwrap().behaviors.iter().filter(|b| &b.function_id == f).cloned().collect())
    }
    async fn upsert_scenario(&self, s: TestScenario) -> KResult<()> {
        upsert_by(&mut self.inner.write().unwrap().scenarios, s, |a, b| a.id == b.id);
        Ok(())
    }
    async fn scenarios_for(&self, f: &FunctionId) -> KResult<Vec<TestScenario>> {
        Ok(self.inner.read().unwrap().scenarios.iter().filter(|s| &s.function_id == f).cloned().collect())
    }
    async fn upsert_decision(&self, d: ArchitectureDecision) -> KResult<()> {
        upsert_by(&mut self.inner.write().unwrap().decisions, d, |a, b| a.id == b.id);
        Ok(())
    }
    async fn decisions_for(&self, f: &FunctionId) -> KResult<Vec<ArchitectureDecision>> {
        Ok(self.inner.read().unwrap().decisions.iter().filter(|d| &d.function_id == f).cloned().collect())
    }
    async fn add_relationship(&self, r: Relationship) -> KResult<()> {
        let mut g = self.inner.write().unwrap();
        if !g.relationships.contains(&r) {
            g.relationships.push(r);
        }
        Ok(())
    }
    async fn relationships(&self) -> KResult<Vec<Relationship>> {
        Ok(self.inner.read().unwrap().relationships.clone())
    }
    async fn record_evidence(&self, e: EvidenceRecord) -> KResult<()> {
        self.inner.write().unwrap().evidence.push(e);
        Ok(())
    }
    async fn evidence_for(&self, f: &FunctionId) -> KResult<Vec<EvidenceRecord>> {
        Ok(self.inner.read().unwrap().evidence.iter().filter(|e| &e.function_id == f).cloned().collect())
    }
    async fn queue_review(&self, r: ReviewRequest) -> KResult<()> {
        self.reviews.write().unwrap().insert(r.id.clone(), r);
        Ok(())
    }
    async fn list_reviews(&self) -> KResult<Vec<ReviewRequest>> {
        Ok(self.reviews.read().unwrap().values().cloned().collect())
    }
    async fn decide_review(&self, id: &str, status: ReviewStatus, by: &str) -> KResult<()> {
        let mut r = self.reviews.write().unwrap();
        let req = r.get_mut(id).ok_or_else(|| KnowledgeError::NotFound(id.to_string()))?;
        req.status = status;
        req.decided_by = Some(by.to_string());
        Ok(())
    }
    async fn snapshot(&self) -> KResult<KnowledgeSnapshot> {
        Ok(self.inner.read().unwrap().clone())
    }
}
