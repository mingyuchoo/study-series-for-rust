//! Boundary Generation Agent (design §13): deterministic boundary values from rule conditions.
use amap_domain::*;
use amap_invariant::{BinOp, Expr};
use amap_orchestrator::{AgentContext, AgentResult, AgentTask, OrchestrationError};
use async_trait::async_trait;
use serde_json::{json, Value};

#[derive(Default)]
pub struct BoundaryAgent;

/// `(path, candidate values)` pairs discovered in an expression.
pub fn boundary_sets(expr: &Expr) -> Vec<(Vec<String>, Vec<Value>)> {
    let mut out = Vec::new();
    collect(expr, &mut out);
    out
}

fn collect(e: &Expr, out: &mut Vec<(Vec<String>, Vec<Value>)>) {
    if let Expr::Binary(op, l, r) = e {
        match (op, l.as_ref(), r.as_ref()) {
            (BinOp::And | BinOp::Or, _, _) => {
                collect(l, out);
                collect(r, out);
            }
            (BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne, Expr::Path(p), Expr::Num(n)) | (BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne, Expr::Num(n), Expr::Path(p)) => {
                let step = if n.fract() == 0.0 { 1.0 } else { 0.01 };
                out.push((p.clone(), vec![json!(n - step), json!(*n), json!(n + step)]));
            }
            (BinOp::Eq | BinOp::Ne, Expr::Path(p), Expr::Str(s)) | (BinOp::Eq | BinOp::Ne, Expr::Str(s), Expr::Path(p)) => {
                out.push((p.clone(), vec![json!(s), json!(format!("NOT_{s}"))]));
            }
            _ => {}
        }
    }
    if let Expr::Unary(_, inner) = e {
        collect(inner, out);
    }
}

fn set_path(target: &mut Value, path: &[String], value: Value) -> bool {
    if path.is_empty() {
        return false;
    }
    let Value::Object(map) = target else { return false };
    if path.len() == 1 {
        map.insert(path[0].clone(), value);
        return true;
    }
    match map.get_mut(&path[0]) {
        Some(child) => set_path(child, &path[1..], value),
        None => false,
    }
}

fn cartesian(sets: &[(Vec<String>, Vec<Value>)], cap: usize) -> Vec<Vec<(Vec<String>, Value)>> {
    let mut combos: Vec<Vec<(Vec<String>, Value)>> = vec![vec![]];
    for (path, values) in sets {
        let mut next = Vec::new();
        for c in &combos {
            for v in values {
                let mut n = c.clone();
                n.push((path.clone(), v.clone()));
                next.push(n);
                if next.len() >= cap {
                    break;
                }
            }
        }
        combos = next;
    }
    combos
}

#[async_trait]
impl AgentTask for BoundaryAgent {
    fn name(&self) -> &str {
        "boundary"
    }
    fn role(&self) -> AgentRole {
        AgentRole::Boundary
    }
    async fn execute(&self, ctx: &AgentContext) -> Result<AgentResult, OrchestrationError> {
        let rules = ctx.knowledge.rules_for(&ctx.function_id).await?;
        let behaviors = ctx.knowledge.behaviors_for(&ctx.function_id).await?;
        let bases: Vec<&BehaviorRecord> = behaviors.iter().take(2).collect();
        if bases.is_empty() {
            return Ok(AgentResult::new("no base behaviors for boundary generation", json!({ "scenarios": 0 })));
        }
        let mut count = 0;
        for rule in &rules {
            let Ok(cond) = amap_invariant::parse_expr(&rule.condition) else { continue };
            let sets = boundary_sets(&cond);
            if sets.is_empty() {
                continue;
            }
            for (bi, base) in bases.iter().enumerate() {
                for (ci, combo) in cartesian(&sets, 27).into_iter().enumerate() {
                    let mut input = base.input.clone();
                    let mut applied = 0;
                    for (path, value) in &combo {
                        if set_path(&mut input, path, value.clone()) {
                            applied += 1;
                        }
                    }
                    if applied == 0 {
                        continue;
                    }
                    let scenario = TestScenario {
                        id: ScenarioId::new(format!("TEST-BND-{}-{}-{:03}", rule.id.0.rsplit('-').next().unwrap_or("r"), bi, ci)),
                        function_id: ctx.function_id.clone(),
                        origin: ScenarioOrigin::Boundary,
                        rule_ids: vec![rule.id.clone()],
                        initial_state: base.initial_state.clone(),
                        input,
                        expected_output: None,
                        expected_state_change: None,
                        expected_events: vec![],
                        priority: rule.priority,
                        comparator_spec: ctx.config.default_spec.clone(),
                        behavior_id: None,
                    };
                    ctx.knowledge.upsert_scenario(scenario).await?;
                    count += 1;
                }
            }
        }
        Ok(AgentResult::new(format!("generated {count} boundary scenarios"), json!({ "scenarios": count })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_from_condition() {
        let e = amap_invariant::parse_expr(r#"age >= 65 AND balance > 100000000 AND grade == "VIP""#).unwrap();
        let sets = boundary_sets(&e);
        assert_eq!(sets.len(), 3);
        assert_eq!(sets[0].1, vec![json!(64.0), json!(65.0), json!(66.0)]);
        assert_eq!(cartesian(&sets, 100).len(), 18);
    }
}
