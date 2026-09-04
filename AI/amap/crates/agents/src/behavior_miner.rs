//! Behavior Mining Agent (design §5): production traces → Behavior Records → Golden Master.
//! Rules are linked to behaviors *deterministically* by evaluating their DSL conditions.
use crate::rule_miner::{rule_as_invariant, rule_matches};
use amap_domain::*;
use amap_invariant::check;
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError, TraceSource};
use amap_uncertainty::{rule_confidence, EvidenceSignals};
use async_trait::async_trait;
#[cfg(feature = "kafka")]
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
#[cfg(feature = "kafka")]
use rdkafka::{ClientConfig, Message};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
#[cfg(feature = "kafka")]
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

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
    let output = v
        .get("response")
        .or_else(|| v.get("legacy_output"))?
        .clone();
    Some(BehaviorRecord {
        id: BehaviorId::new(format!("BH-{:08}", n)),
        function_id: function_id.clone(),
        initial_state: v
            .get("before")
            .or_else(|| v.get("initial_state"))
            .cloned()
            .unwrap_or(json!({})),
        input,
        legacy_output: output,
        db_state_change: v
            .get("after")
            .or_else(|| v.get("db_state_change"))
            .cloned()
            .unwrap_or(Value::Null),
        events: v
            .get("events")
            .and_then(|e| e.as_array().cloned())
            .unwrap_or_default(),
        external_calls: v
            .get("external_calls")
            .and_then(|e| e.as_array().cloned())
            .unwrap_or_default(),
        timing_ms: v.get("timing_ms").and_then(|t| t.as_u64()),
        related_rules: vec![],
        priority: v
            .get("priority")
            .and_then(|p| serde_json::from_value(p.clone()).ok()),
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

async fn persist_behavior(
    ctx: &AgentContext,
    rules: &[BusinessRule],
    observed: &mut HashMap<String, (u64, u64)>,
    mut behavior: BehaviorRecord,
) -> Result<usize, OrchestrationError> {
    let view = behavior_view(&behavior);
    behavior.related_rules.clear();
    let mut linked = 0;
    for rule in rules {
        if rule_matches(rule, &view) == Some(true) {
            behavior.related_rules.push(rule.id.clone());
            let evidence = observed.entry(rule.id.0.clone()).or_default();
            evidence.0 += 1;
            if let Some(invariant) = rule_as_invariant(rule) {
                if check(&invariant, &view).holds {
                    evidence.1 += 1;
                }
            }
            linked += 1;
        }
    }
    let golden = amap_replay::scenarios_from_behaviors(
        std::slice::from_ref(&behavior),
        ctx.config.default_spec.as_deref(),
        Priority::P1,
    );
    ctx.knowledge.upsert_behavior(behavior).await?;
    for scenario in golden {
        ctx.knowledge.upsert_scenario(scenario).await?;
    }
    Ok(linked)
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
        if ctx.config.traces.is_none() && ctx.config.trace_sources.is_empty() {
            return Ok(AgentResult::new(
                "no production traces configured",
                json!({ "behaviors": 0 }),
            ));
        }
        let mut rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let mut observed: HashMap<String, (u64, u64)> = HashMap::new(); // rule → (matched, result_holds)
        let mut behaviors = 0usize;
        let mut linked = 0usize;
        let mut malformed = 0usize;
        let mut file_paths = Vec::new();
        if let Some(path) = &ctx.config.traces {
            file_paths.push(path.clone());
        }
        file_paths.extend(
            ctx.config
                .trace_sources
                .iter()
                .filter_map(|source| match source {
                    TraceSource::File { path } => Some(path.clone()),
                    TraceSource::Kafka { .. } => None,
                }),
        );
        let mut seen_files = HashSet::new();
        for path in file_paths {
            if !seen_files.insert(path.clone()) {
                continue;
            }
            let file = tokio::fs::File::open(&path)
                .await
                .map_err(|error| OrchestrationError::agent("behavior_mining", error))?;
            let mut lines = BufReader::new(file).lines();
            while let Some(line) = lines
                .next_line()
                .await
                .map_err(|error| OrchestrationError::agent("behavior_mining", error))?
            {
                if line.trim().is_empty() {
                    continue;
                }
                let Some(behavior) = parse_trace_line(&line, &ctx.function_id, behaviors + 1)
                else {
                    malformed += 1;
                    continue;
                };
                linked += persist_behavior(ctx, &rules, &mut observed, behavior).await?;
                behaviors += 1;
            }
        }

        for source in &ctx.config.trace_sources {
            let TraceSource::Kafka {
                brokers,
                topic,
                group_id,
                max_records,
                idle_timeout_ms,
            } = source
            else {
                continue;
            };
            #[cfg(not(feature = "kafka"))]
            {
                let _ = (brokers, topic, group_id, max_records, idle_timeout_ms);
                return Err(OrchestrationError::agent(
                    "behavior_mining",
                    "Kafka trace ingestion requires the `amap-agents/kafka` feature",
                ));
            }
            #[cfg(feature = "kafka")]
            {
                let mut kafka = ClientConfig::new();
                kafka
                    .set("bootstrap.servers", brokers)
                    .set("group.id", group_id)
                    .set("enable.auto.commit", "false")
                    .set("auto.offset.reset", "earliest");
                for (property, variable) in [
                    ("security.protocol", "AMAP_KAFKA_SECURITY_PROTOCOL"),
                    ("sasl.mechanism", "AMAP_KAFKA_SASL_MECHANISM"),
                    ("sasl.username", "AMAP_KAFKA_SASL_USERNAME"),
                    ("sasl.password", "AMAP_KAFKA_SASL_PASSWORD"),
                    ("ssl.ca.location", "AMAP_KAFKA_SSL_CA_LOCATION"),
                ] {
                    if let Ok(value) = std::env::var(variable) {
                        kafka.set(property, value);
                    }
                }
                let consumer: StreamConsumer = kafka
                    .create()
                    .map_err(|error| OrchestrationError::agent("behavior_mining", error))?;
                consumer
                    .subscribe(&[topic])
                    .map_err(|error| OrchestrationError::agent("behavior_mining", error))?;
                let mut consumed = 0usize;
                while consumed < *max_records {
                    let message = match tokio::time::timeout(
                        Duration::from_millis(*idle_timeout_ms),
                        consumer.recv(),
                    )
                    .await
                    {
                        Ok(Ok(message)) => message,
                        Ok(Err(error)) => {
                            return Err(OrchestrationError::agent("behavior_mining", error));
                        }
                        Err(_) => break,
                    };
                    consumed += 1;
                    let line = match message.payload_view::<str>() {
                        Some(Ok(payload)) => payload,
                        _ => {
                            malformed += 1;
                            consumer
                                .commit_message(&message, CommitMode::Sync)
                                .map_err(|error| {
                                    OrchestrationError::agent("behavior_mining", error)
                                })?;
                            continue;
                        }
                    };
                    if let Some(mut behavior) =
                        parse_trace_line(line, &ctx.function_id, behaviors + 1)
                    {
                        behavior.id = BehaviorId::new(format!(
                            "BH-KAFKA-{}-{}-{}",
                            topic,
                            message.partition(),
                            message.offset()
                        ));
                        linked += persist_behavior(ctx, &rules, &mut observed, behavior).await?;
                        behaviors += 1;
                    } else {
                        malformed += 1;
                    }
                    consumer
                        .commit_message(&message, CommitMode::Sync)
                        .map_err(|error| OrchestrationError::agent("behavior_mining", error))?;
                }
            }
        }

        // Update rule confidence from production evidence (deterministic; design §15).
        let mut contradicted = Vec::new();
        for r in rules.iter_mut() {
            let (matched, holds) = observed.get(&r.id.0).copied().unwrap_or((0, 0));
            r.observed_production_cases = matched;
            let mut ev = r.evidence;
            ev.production = matched > 0;
            let mut signals = EvidenceSignals {
                production_evidence_exists: matched > 0,
                ..Default::default()
            };
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
        ctx.emit(
            "behaviors.mined",
            json!({ "behaviors": behaviors, "links": linked, "malformed": malformed }),
        )
        .await;
        Ok(AgentResult::new(
            format!("mined {behaviors} production behaviors → golden scenarios, {linked} rule links, {} contradictions", contradicted.len()),
            json!({ "behaviors": behaviors, "links": linked, "malformed": malformed, "contradicted": contradicted }),
        ))
    }
}
