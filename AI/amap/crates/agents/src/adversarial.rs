//! Adversarial Agent (design §14): "how can this system be broken?" — independent context,
//! a different model from the builder, plus deterministic hostile probes.
use crate::context_pack;
use amap_context::ContextBudget;
use amap_domain::*;
use amap_llm::{LlmRequest, TaskKind};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use async_trait::async_trait;
use serde_json::{json, Value};

#[derive(Default)]
pub struct AdversarialAgent;

/// Deterministic probes derived from a base input: every leaf gets nulled, negated, maxed, etc.
pub fn builtin_probes(base_input: &Value, base_state: &Value) -> Vec<(String, Value, Value)> {
    let mut probes = Vec::new();
    let leaves = leaf_paths(base_input, vec![]);
    for path in &leaves {
        let mut v = base_input.clone();
        set(&mut v, path, Value::Null);
        probes.push((format!("null:{}", path.join(".")), v, base_state.clone()));
        if let Some(Value::Number(n)) = get(base_input, path) {
            let f = n.as_f64().unwrap_or(0.0);
            for (label, val) in [
                ("negative", json!(-f.abs().max(1.0))),
                ("zero", json!(0)),
                ("max", json!(9007199254740991i64)),
                ("fraction", json!(f + 0.001)),
            ] {
                let mut v = base_input.clone();
                set(&mut v, path, val);
                probes.push((format!("{label}:{}", path.join(".")), v, base_state.clone()));
            }
        }
        if let Some(Value::String(s)) = get(base_input, path) {
            for (label, val) in [
                ("empty", json!("")),
                ("unicode", json!(format!("{s}✓한글"))),
                ("long", json!("x".repeat(4096))),
            ] {
                let mut v = base_input.clone();
                set(&mut v, path, val);
                probes.push((format!("{label}:{}", path.join(".")), v, base_state.clone()));
            }
            if s.contains('-') && s.len() >= 10 {
                for (label, date) in [
                    ("leap_day", "2024-02-29"),
                    ("year_end", "2025-12-31"),
                    ("month_end", "2026-02-28"),
                    ("out_of_order", "1999-01-01"),
                ] {
                    let mut v = base_input.clone();
                    set(&mut v, path, json!(date));
                    probes.push((format!("{label}:{}", path.join(".")), v, base_state.clone()));
                }
            }
        }
    }
    // Duplicate request: the same request id is already recorded as processed.
    if let Some(Value::String(rid)) = base_input.get("request_id") {
        let mut st = base_state.clone();
        if let Value::Object(m) = &mut st {
            m.insert("processed_request_ids".into(), json!([rid]));
        }
        probes.push(("duplicate_request".into(), base_input.clone(), st));
    }
    probes
}

fn leaf_paths(v: &Value, prefix: Vec<String>) -> Vec<Vec<String>> {
    match v {
        Value::Object(m) => m
            .iter()
            .flat_map(|(k, v)| {
                let mut p = prefix.clone();
                p.push(k.clone());
                leaf_paths(v, p)
            })
            .collect(),
        _ => vec![prefix],
    }
}
fn get<'a>(v: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter().try_fold(v, |acc, k| acc.get(k))
}
fn set(v: &mut Value, path: &[String], val: Value) {
    if path.len() == 1 {
        if let Value::Object(m) = v {
            m.insert(path[0].clone(), val);
        }
    } else if let Some(child) = v.get_mut(&path[0]) {
        set(child, &path[1..], val);
    }
}

#[async_trait]
impl AgentTask for AdversarialAgent {
    fn name(&self) -> &str {
        "adversarial"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Adversarial
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let behaviors = ctx.knowledge.behaviors_for(&ctx.function_id).await?;
        let Some(base) = behaviors.first() else {
            return Ok(AgentResult::new(
                "no base behavior for adversarial probing",
                json!({ "scenarios": 0 }),
            ));
        };
        let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let mut count = 0;
        let mut store = |mut input: Value,
                         state: Value,
                         rule_ids: Vec<RuleId>,
                         unique_request: bool,
                         ctx: &AgentContext| {
            let id = ScenarioId::new(format!("TEST-ADV-{:04}", count + 1));
            count += 1;
            if unique_request {
                if let Some(object) = input.as_object_mut() {
                    if object.contains_key("request_id") {
                        object.insert("request_id".into(), json!(format!("ADV-{}", id.0)));
                    }
                }
            }
            TestScenario {
                id,
                function_id: ctx.function_id.clone(),
                origin: ScenarioOrigin::Adversarial,
                rule_ids,
                initial_state: state,
                input,
                expected_output: None,
                expected_state_change: None,
                expected_events: None,
                expected_external_calls: None,
                expected_timing_ms: None,
                priority: Priority::P1,
                comparator_spec: ctx.config.default_spec.clone(),
                behavior_id: None,
            }
        };
        let mut scenarios = Vec::new();
        for (name, input, state) in builtin_probes(&base.input, &base.initial_state) {
            let targets_request_id = name.ends_with(":request_id");
            let duplicate_request = name == "duplicate_request";
            scenarios.push((
                name,
                store(
                    input,
                    state,
                    vec![],
                    !targets_request_id && !duplicate_request,
                    ctx,
                ),
            ));
        }

        // LLM probes from a model different from the builder's.
        let builder_provider: Option<ModelProvider> =
            serde_json::from_value(ctx.input("build")["provider"].clone()).ok();
        let mut pack =
            context_pack(ctx, "adversarial edge cases", ContextBudget::default()).await?;
        pack.source.clear();
        let mut req = LlmRequest::new(
            TaskKind::AdversarialProbe,
            AgentRole::Adversarial,
            "",
            format!("{}\n\nBase input: {}", pack.render(), base.input),
        )
        .with_run(ctx.run_id.0.clone());
        if let Some(bp) = builder_provider {
            for candidate in [
                ModelProvider::Anthropic,
                ModelProvider::OpenAi,
                ModelProvider::Local,
                ModelProvider::Mock,
            ] {
                if candidate != bp {
                    req = req.with_provider(candidate);
                    break;
                }
            }
        }
        let llm_probes = match ctx.llm.complete(req.clone()).await {
            Ok(r) => r
                .json
                .and_then(|j| j["probes"].as_array().cloned())
                .unwrap_or_default(),
            Err(_) => match ctx
                .llm
                .complete(LlmRequest {
                    provider: None,
                    ..req
                })
                .await
            {
                Ok(r) => r
                    .json
                    .and_then(|j| j["probes"].as_array().cloned())
                    .unwrap_or_default(),
                Err(e) => {
                    tracing::warn!(error = %e, "LLM adversarial probes unavailable; using built-in probes only");
                    vec![]
                }
            },
        };
        for p in &llm_probes {
            if p["input"].is_object() {
                let rule_ids = crate::str_list(&p["rule_ids"])
                    .into_iter()
                    .filter(|candidate| rules.iter().any(|rule| rule.id.0 == *candidate))
                    .map(RuleId::new)
                    .collect();
                scenarios.push((
                    p["name"].as_str().unwrap_or("llm").to_string(),
                    store(
                        p["input"].clone(),
                        p["initial_state"].clone(),
                        rule_ids,
                        false,
                        ctx,
                    ),
                ));
            }
        }
        let n = scenarios.len();
        for (_, s) in scenarios {
            ctx.knowledge.upsert_scenario(s).await?;
        }
        Ok(AgentResult::new(
            format!(
                "generated {n} adversarial probes ({} from LLM)",
                llm_probes.len()
            ),
            json!({ "scenarios": n, "llm_probes": llm_probes.len() }),
        ))
    }
}
