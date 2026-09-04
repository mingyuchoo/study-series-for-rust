//! Graph projection of the System Knowledge Graph (petgraph) for traversal:
//! `Requirement → Function → Rule → Source/DB/Interface → Behavior → Scenario → Evidence`.

use amap_domain::*;
use amap_knowledge::KnowledgeSnapshot;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Bfs, EdgeRef, Reversed};
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Requirement,
    Function,
    Rule,
    Source,
    Db,
    Interface,
    Behavior,
    Scenario,
    Evidence,
    Service,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
}

pub struct KnowledgeGraph {
    graph: DiGraph<Node, RelationKind>,
    index: HashMap<String, NodeIndex>,
}

impl KnowledgeGraph {
    pub fn from_snapshot(s: &KnowledgeSnapshot) -> Self {
        let mut g = Self {
            graph: DiGraph::new(),
            index: HashMap::new(),
        };
        for f in &s.functions {
            g.node(&f.id.0, NodeKind::Function);
        }
        for r in &s.requirements {
            g.node(&r.id.0, NodeKind::Requirement);
            g.edge(
                &r.id.0,
                &r.function_id.0,
                RelationKind::RequirementToFunction,
            );
        }
        for r in &s.rules {
            g.node(&r.id.0, NodeKind::Rule);
            g.edge(&r.function_id.0, &r.id.0, RelationKind::FunctionToRule);
            for requirement in &r.requirement_ids {
                g.node(&requirement.0, NodeKind::Requirement);
                g.edge(&requirement.0, &r.id.0, RelationKind::RequirementToRule);
            }
            for src in &r.sources {
                let sid = src.to_string();
                g.node(&sid, NodeKind::Source);
                g.edge(&r.id.0, &sid, RelationKind::RuleToSource);
            }
            for d in &r.db_entities {
                g.node(&d.0, NodeKind::Db);
                g.edge(&r.id.0, &d.0, RelationKind::RuleToDb);
            }
            for i in &r.interfaces {
                g.node(&i.0, NodeKind::Interface);
                g.edge(&r.id.0, &i.0, RelationKind::RuleToInterface);
            }
        }
        for e in &s.source_units {
            g.node(&e.id.0, NodeKind::Source);
            for d in &e.dependencies {
                g.node(&d.0, NodeKind::Source);
                g.edge(&e.id.0, &d.0, RelationKind::SourceCalls);
            }
        }
        for b in &s.behaviors {
            g.node(&b.id.0, NodeKind::Behavior);
            for r in &b.related_rules {
                g.node(&r.0, NodeKind::Rule);
                g.edge(&r.0, &b.id.0, RelationKind::RuleToBehavior);
            }
        }
        for t in &s.scenarios {
            g.node(&t.id.0, NodeKind::Scenario);
            if let Some(b) = &t.behavior_id {
                g.node(&b.0, NodeKind::Behavior);
                g.edge(&b.0, &t.id.0, RelationKind::BehaviorToScenario);
            }
            for r in &t.rule_ids {
                g.node(&r.0, NodeKind::Rule);
                g.edge(&r.0, &t.id.0, RelationKind::RuleToBehavior);
            }
        }
        for (i, e) in s.evidence.iter().enumerate() {
            let eid = format!("EV-{}-{}", e.run_id.0, i);
            g.node(&eid, NodeKind::Evidence);
            g.node(&e.scenario_id, NodeKind::Scenario);
            g.edge(&e.scenario_id, &eid, RelationKind::ScenarioToEvidence);
        }
        for d in &s.decisions {
            for (rule, service) in &d.rule_ownership {
                g.node(service, NodeKind::Service);
                g.edge(&rule.0, service, RelationKind::RuleOwnedByService);
            }
        }
        for rel in &s.relationships {
            let kind_from = match rel.kind {
                RelationKind::SourceCalls
                | RelationKind::SourceReadsDb
                | RelationKind::SourceWritesDb => NodeKind::Source,
                _ => NodeKind::Rule,
            };
            let kind_to = match rel.kind {
                RelationKind::SourceReadsDb | RelationKind::SourceWritesDb => NodeKind::Db,
                RelationKind::SourceCalls => NodeKind::Source,
                _ => NodeKind::Rule,
            };
            g.node(&rel.from, kind_from);
            g.node(&rel.to, kind_to);
            g.edge(&rel.from, &rel.to, rel.kind);
        }
        g
    }

    fn node(&mut self, id: &str, kind: NodeKind) -> NodeIndex {
        if let Some(ix) = self.index.get(id) {
            return *ix;
        }
        let ix = self.graph.add_node(Node {
            id: id.to_string(),
            kind,
        });
        self.index.insert(id.to_string(), ix);
        ix
    }

    fn edge(&mut self, from: &str, to: &str, kind: RelationKind) {
        let (Some(&a), Some(&b)) = (self.index.get(from), self.index.get(to)) else {
            return;
        };
        if !self
            .graph
            .edges_connecting(a, b)
            .any(|e| *e.weight() == kind)
        {
            self.graph.add_edge(a, b, kind);
        }
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Everything reachable downstream from `id` (impact analysis: which scenarios / evidence a rule touches).
    pub fn downstream(&self, id: &str) -> Vec<Node> {
        self.walk(id, Direction::Outgoing)
    }

    /// Everything upstream of `id` (lineage: which requirement / rule explains a scenario).
    pub fn upstream(&self, id: &str) -> Vec<Node> {
        self.walk(id, Direction::Incoming)
    }

    fn walk(&self, id: &str, dir: Direction) -> Vec<Node> {
        let Some(&start) = self.index.get(id) else {
            return vec![];
        };
        let mut out = Vec::new();
        match dir {
            Direction::Outgoing => {
                let mut bfs = Bfs::new(&self.graph, start);
                while let Some(n) = bfs.next(&self.graph) {
                    if n != start {
                        out.push(self.graph[n].clone());
                    }
                }
            }
            Direction::Incoming => {
                let rev = Reversed(&self.graph);
                let mut bfs = Bfs::new(rev, start);
                while let Some(n) = bfs.next(rev) {
                    if n != start {
                        out.push(self.graph[n].clone());
                    }
                }
            }
        }
        out
    }

    /// Scenarios affected by a rule (used to scope RCA / re-verification).
    pub fn affected_scenarios(&self, rule_id: &str) -> Vec<String> {
        self.downstream(rule_id)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Scenario)
            .map(|n| n.id)
            .collect()
    }

    /// Rules with no source evidence, no behavior evidence or no scenario — the "unknown-risk candidates".
    pub fn weakly_evidenced_rules(&self) -> Vec<(String, Vec<&'static str>)> {
        let mut out = Vec::new();
        for (id, &ix) in &self.index {
            if self.graph[ix].kind != NodeKind::Rule {
                continue;
            }
            let kinds: HashSet<NodeKind> = self
                .graph
                .neighbors_directed(ix, Direction::Outgoing)
                .map(|n| self.graph[n].kind)
                .collect();
            let mut missing = Vec::new();
            if !kinds.contains(&NodeKind::Source) {
                missing.push("source");
            }
            if !kinds.contains(&NodeKind::Behavior) {
                missing.push("behavior");
            }
            if !kinds.contains(&NodeKind::Scenario) {
                missing.push("scenario");
            }
            if !missing.is_empty() {
                out.push((id.clone(), missing));
            }
        }
        out.sort();
        out
    }

    /// Graphviz DOT rendering for dashboards / audits.
    pub fn to_dot(&self) -> String {
        let mut s = String::from("digraph amap {\n  rankdir=LR;\n");
        for n in self.graph.node_weights() {
            s.push_str(&format!(
                "  \"{}\" [shape=box,label=\"{}\\n{:?}\"];\n",
                n.id, n.id, n.kind
            ));
        }
        for e in self.graph.edge_references() {
            s.push_str(&format!(
                "  \"{}\" -> \"{}\" [label=\"{:?}\"];\n",
                self.graph[e.source()].id,
                self.graph[e.target()].id,
                e.weight()
            ));
        }
        s.push_str("}\n");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn impact_and_lineage() {
        let fid = FunctionId::new("FN-1");
        let rule = BusinessRule {
            id: RuleId::new("BR-1"),
            function_id: fid.clone(),
            name: "r".into(),
            condition: "true".into(),
            result: "true".into(),
            sources: vec![SourceLocation::new("A.cbl", 1, 2)],
            db_entities: vec![],
            interfaces: vec![],
            requirement_ids: vec![RequirementId::new("REQ-1")],
            observed_production_cases: 1,
            evidence: Default::default(),
            confidence: 0.9,
            priority: Priority::P0,
            tags: vec![],
        };
        let beh = BehaviorRecord {
            id: BehaviorId::new("BH-1"),
            function_id: fid.clone(),
            initial_state: json!({}),
            input: json!({}),
            legacy_output: json!({}),
            db_state_change: json!({}),
            events: vec![],
            external_calls: vec![],
            timing_ms: None,
            related_rules: vec![RuleId::new("BR-1")],
            priority: None,
        };
        let sc = TestScenario {
            id: ScenarioId::new("TEST-1"),
            function_id: fid.clone(),
            origin: ScenarioOrigin::GoldenMaster,
            rule_ids: vec![RuleId::new("BR-1")],
            initial_state: json!({}),
            input: json!({}),
            expected_output: None,
            expected_state_change: None,
            expected_events: Some(vec![]),
            expected_external_calls: Some(vec![]),
            expected_timing_ms: None,
            priority: Priority::P0,
            comparator_spec: None,
            behavior_id: Some(BehaviorId::new("BH-1")),
        };
        let snap = KnowledgeSnapshot {
            functions: vec![BusinessFunction {
                id: fid.clone(),
                name: "f".into(),
                domain: "loan".into(),
                priority: Priority::P0,
                description: String::new(),
            }],
            requirements: vec![Requirement {
                id: RequirementId::new("REQ-1"),
                function_id: fid,
                title: "t".into(),
                text: "x".into(),
                confidence: 0.9,
            }],
            rules: vec![rule],
            behaviors: vec![beh],
            scenarios: vec![sc],
            ..Default::default()
        };
        let g = KnowledgeGraph::from_snapshot(&snap);
        assert_eq!(g.affected_scenarios("BR-1"), vec!["TEST-1".to_string()]);
        assert!(g.upstream("TEST-1").iter().any(|n| n.id == "REQ-1"));
        assert!(g.weakly_evidenced_rules().is_empty());
    }
}
