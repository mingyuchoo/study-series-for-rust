use crate::builtin::*;
use crate::spec::{ComparatorKind, ComparatorSpec, FieldRule};
use crate::{Comparator, ComparatorError, ComparisonContext};
use amap_domain::Difference;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub equal: bool,
    pub differences: Vec<Difference>,
    pub fields_compared: usize,
}

/// Registry of comparators + spec-driven structural walker.
pub struct ComparisonEngine {
    plugins: HashMap<String, Arc<dyn Comparator>>,
    exact: ExactComparator,
    numeric: NumericComparator,
    timestamp: TimestampComparator,
    format: FormatComparator,
    unordered: UnorderedComparator,
    ignore: IgnoreComparator,
}

impl Default for ComparisonEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ComparisonEngine {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            exact: ExactComparator,
            numeric: NumericComparator,
            timestamp: TimestampComparator,
            format: FormatComparator,
            unordered: UnorderedComparator,
            ignore: IgnoreComparator,
        }
    }

    /// Register a custom (e.g. WASM) comparator addressable via `comparator: plugin, plugin: <name>`.
    pub fn register_plugin(&mut self, name: impl Into<String>, comparator: Arc<dyn Comparator>) {
        self.plugins.insert(name.into(), comparator);
    }

    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Compare `expected` (legacy) with `actual` (next) under `spec`.
    pub fn compare(&self, spec: &ComparatorSpec, expected: &Value, actual: &Value) -> Result<ComparisonResult, ComparatorError> {
        let seen = Mutex::new(HashSet::new());
        let mut result = ComparisonResult { equal: true, differences: Vec::new(), fields_compared: 0 };
        self.walk(spec, "", expected, actual, &seen, &mut result)?;
        result.equal = result.differences.is_empty();
        Ok(result)
    }

    fn walk(
        &self,
        spec: &ComparatorSpec,
        path: &str,
        expected: &Value,
        actual: &Value,
        seen: &Mutex<HashSet<String>>,
        out: &mut ComparisonResult,
    ) -> Result<(), ComparatorError> {
        let default_rule = FieldRule { comparator: spec.default, ..FieldRule::exact() };
        let rule = spec.rule_for(path).unwrap_or(&default_rule);

        // Exact rules descend structurally so nested rules still apply.
        let descend = rule.comparator == ComparatorKind::Exact;
        if descend {
            match (expected, actual) {
                (Value::Object(e), Value::Object(a)) => {
                    let mut keys: Vec<&String> = e.keys().chain(a.keys()).collect();
                    keys.sort();
                    keys.dedup();
                    for k in keys {
                        let child = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                        match (e.get(k), a.get(k)) {
                            (Some(ev), Some(av)) => self.walk(spec, &child, ev, av, seen, out)?,
                            (Some(ev), None) => {
                                if !self.is_ignored(spec, &child) {
                                    out.differences.push(diff(&child, ev.clone(), Value::Null, "exact", "missing in actual"));
                                }
                            }
                            (None, Some(av)) => {
                                if !self.is_ignored(spec, &child) {
                                    out.differences.push(diff(&child, Value::Null, av.clone(), "exact", "unexpected in actual"));
                                }
                            }
                            (None, None) => {}
                        }
                    }
                    return Ok(());
                }
                (Value::Array(e), Value::Array(a)) => {
                    if e.len() != a.len() {
                        out.differences.push(diff(path, expected.clone(), actual.clone(), "exact", &format!("array length {} vs {}", e.len(), a.len())));
                        return Ok(());
                    }
                    for (i, (ev, av)) in e.iter().zip(a).enumerate() {
                        let child = format!("{path}[{i}]");
                        self.walk(spec, &child, ev, av, seen, out)?;
                    }
                    return Ok(());
                }
                _ => {}
            }
        }

        out.fields_compared += 1;
        let ctx = ComparisonContext { path, rule, seen_unique: seen };
        let comparator: &dyn Comparator = match rule.comparator {
            ComparatorKind::Exact => &self.exact,
            ComparatorKind::Numeric => &self.numeric,
            ComparatorKind::Tolerance => &self.timestamp,
            ComparatorKind::Format => &self.format,
            ComparatorKind::Unordered => &self.unordered,
            ComparatorKind::Ignore => &self.ignore,
            ComparatorKind::Plugin => {
                let name = rule.plugin.as_deref().unwrap_or_default();
                self.plugins.get(name).map(|c| c.as_ref()).ok_or_else(|| ComparatorError::Unknown(name.to_string()))?
            }
        };
        if let Some(msg) = comparator.compare(expected, actual, &ctx) {
            out.differences.push(diff(path, expected.clone(), actual.clone(), comparator.name(), &msg));
        }
        Ok(())
    }

    fn is_ignored(&self, spec: &ComparatorSpec, path: &str) -> bool {
        spec.rule_for(path).map(|r| r.comparator == ComparatorKind::Ignore).unwrap_or(false)
    }
}

fn diff(path: &str, expected: Value, actual: Value, comparator: &str, message: &str) -> Difference {
    Difference { path: path.to_string(), expected, actual, comparator: comparator.to_string(), message: message.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec() -> ComparatorSpec {
        ComparatorSpec::from_yaml(
            r#"
name: loan
fields:
  amount: { comparator: exact }
  interest: { comparator: numeric, tolerance: 0.005 }
  timestamp: { comparator: tolerance, tolerance: 2s }
  transaction_id: { comparator: format, pattern: "^TX-[0-9A-F]{8}$", unique: true }
  events: { comparator: unordered }
  trace_id: { comparator: ignore }
"#,
        )
        .unwrap()
    }

    #[test]
    fn equivalent_under_policy() {
        let e = json!({"amount": 100, "interest": 12.341, "timestamp": "2026-01-01T00:00:00Z",
                       "transaction_id": "TX-AAAAAAAA", "events": [1,2,3], "trace_id": "legacy"});
        let a = json!({"amount": 100.0, "interest": 12.344, "timestamp": "2026-01-01T00:00:01Z",
                       "transaction_id": "TX-BBBBBBBB", "events": [3,1,2], "trace_id": "next"});
        let r = ComparisonEngine::new().compare(&spec(), &e, &a).unwrap();
        assert!(r.equal, "{:?}", r.differences);
    }

    #[test]
    fn business_field_difference_is_reported() {
        let e = json!({"amount": 100, "fee": 0});
        let a = json!({"amount": 100, "fee": 1500});
        let r = ComparisonEngine::new().compare(&spec(), &e, &a).unwrap();
        assert!(!r.equal);
        assert_eq!(r.differences[0].path, "fee");
    }

    #[test]
    fn unique_violation() {
        let s = spec();
        let engine = ComparisonEngine::new();
        let e = json!({"transaction_id": "x"});
        let a = json!({"transaction_id": "TX-AAAAAAAA"});
        assert!(engine.compare(&s, &e, &a).unwrap().equal);
        let seen = Mutex::new(HashSet::new());
        let mut out = ComparisonResult::default();
        engine.walk(&s, "", &e, &a, &seen, &mut out).unwrap();
        engine.walk(&s, "", &e, &a, &seen, &mut out).unwrap();
        assert_eq!(out.differences.len(), 1);
    }
}
