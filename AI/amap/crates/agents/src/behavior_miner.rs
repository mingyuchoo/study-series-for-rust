//! Behavior Mining Agent (design §5): production traces → Behavior Records → Golden Master.
//! Rules are linked to behaviors *deterministically* by evaluating their DSL conditions.
use crate::rule_miner::{rule_as_invariant, rule_matches};
use amap_domain::*;
use amap_invariant::check;
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use amap_uncertainty::{rule_confidence, EvidenceSignals};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Default)]
pub struct BehaviorMinerAgent;

/// Accepts either full behavior records or raw captures `{request, response, before, after, events}`.
pub fn parse_trace_line(line: &str, function_id: &FunctionId, n: usize) -> Option<BehaviorRecord> {
    let v: Value = serde_json::from_str(line).ok()?;
    if let Ok(mut b) = serde_json::from_value::<BehaviorRecord>(v.clone()) {
        b.function_id = function_id.clone();
        return Some(b);
    }
    let input = v.get("request").or_else(|| v.get("input"))?.clone();
    let output = v.get("response").or_else(|| v.get("legacy_output"))?.clone();
    Some(BehaviorRecord {
        id: BehaviorId::new(format!("BH-{:08}", n)),
        function_id: function_id.clone(),
        initial_state: v.get("before").or_else(|| v.get("initial_state")).cloned().unwrap_or(json!({})),
        input,
        legacy_output: output,
        db_state_change: v.get("after").or_else(|| v.get("db_state_change")).cloned().unwrap_or(Value::Null),
        events: v.get("events").and_then(|e| e.as_array().cloned()).unwrap_or_default(),
        external_calls: v.get("external_calls").and_then(|e| e.as_array().cloned()).unwrap_or_default(),
        timing_ms: v.get("timing_ms").and_then(|t| t.as_u64()),
        related_rules: vec![],
        priority: v.get("priority").and_then(|p| serde_json::from_value(p.clone()).ok()),
    })
}

/// View of a behavior the DSL sees: output fields flattened + input/state.
pub fn behavior_view(b: &BehaviorRecord) -> Value {
    let mut m = serde_json::Map::new();
    if let Value::Object(o) = &b.legacy_output {
        for (k, v) in o {
            m.insert(k.clone(), v.clone());
        }
    }
    if let Value::Object(o) = &b.input {
        for (k, v) in o {
            m.entry(k.clone()).or_insert(v.clone());
        }
    }
    m.insert("input".into(), b.input.clone());
    m.insert("initial_state".into(), b.initial_state.clone());
    m.insert("output".into(), b.legacy_output.clone());
    m.insert("state_change".into(), b.db_state_change.clone());
    Value::Object(m)
}

#[async_trait]
impl AgentTask for BehaviorMinerAgent {
    fn name(&self) -> &str {
        "behavior_mining"
    }
    fn role(&self) -> AgentRole {
        AgentRole::BehaviorMiner
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let Some(path) = &ctx.config.traces else {
            return Ok(AgentResult::new("no production traces configured", json!({ "behaviors": 0 })));
        };
        let text = std::fs::read_to_string(path).map_err(|e| OrchestrationError::agent("behavior_mining", e))?;
        let mut rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let mut observed: HashMap<String, (u64, u64)> = HashMap::new(); // rule → (matched, result_holds)
        let mut behaviors = 0usize;
        let mut linked = 0usize;
        for (n, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let Some(mut b) = parse_trace_line(line, &ctx.function_id, n + 1) else { continue };
            let view = behavior_view(&b);
            b.related_rules.clear();
            for r in &rules {
                if rule_matches(r, &view) == Some(true) {
                    b.related_rules.push(r.id.clone());
                    let e = observed.entry(r.id.0.clone()).or_default();
                    e.0 += 1;
                    if let Some(inv) = rule_as_invariant(r) {
                        if check(&inv, &view).holds {
                            e.1 += 1;
                        }
                    }
                    linked += 1;
                }
            }
            let golden = amap_replay::scenarios_from_behaviors(std::slice::from_ref(&b), ctx.config.default_spec.as_deref(), Priority::P1);
            ctx.knowledge.upsert_behavior(b).await?;
            for s in golden {
                ctx.knowledge.upsert_scenario(s).await?;
            }
            behaviors += 1;
        }

        // Update rule confidence from production evidence (deterministic; design §15).
        let mut contradicted = Vec::new();
        for r in rules.iter_mut() {
            let (matched, holds) = observed.get(&r.id.0).copied().unwrap_or((0, 0));
            r.observed_production_cases = matched;
            let mut ev = r.evidence;
            ev.production = matched > 0;
            let mut signals = EvidenceSignals { production_evidence_exists: matched > 0, ..Default::default() };
            if matched > 0 && holds == matched {
                signals.source_and_production_agree = true;
            } else if matched > 0 {
                signals.unexplained_diff_exists = true; // legacy contradicts the mined rule → risk signal
                contradicted.push(json!({ "rule": r.id.0, "matched": matched, "holds": holds }));
            }
            r.evidence = ev;
            let mut c = rule_confidence(&ev, &signals);
            if r.tags.iter().any(|t| t == "unparsable") {
                c = c.min(0.40);
            }
            r.confidence = c;
            ctx.knowledge.upsert_rule(r.clone()).await?;
        }
        ctx.emit("behaviors.mined", json!({ "behaviors": behaviors, "links": linked })).await;
        Ok(AgentResult::new(
            format!("mined {behaviors} production behaviors → golden scenarios, {linked} rule links, {} contradictions", contradicted.len()),
            json!({ "behaviors": behaviors, "links": linked, "contradicted": contradicted }),
        ))
    }
}
