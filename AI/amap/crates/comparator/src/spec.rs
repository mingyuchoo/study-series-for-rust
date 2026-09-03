use crate::ComparatorError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Built-in comparator kinds. `plugin` dispatches to a registered (WASM) comparator by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparatorKind {
    Exact,
    Numeric,
    Tolerance,
    Format,
    Unordered,
    Ignore,
    Plugin,
}

/// Equivalence rule for one field path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldRule {
    pub comparator: ComparatorKind,
    /// Numeric tolerance (`numeric`) or duration string such as `2s`, `500ms` (`tolerance`).
    #[serde(default)]
    pub tolerance: Option<serde_yaml::Value>,
    /// Treat numeric tolerance as relative (fraction of expected).
    #[serde(default)]
    pub relative: bool,
    /// Regex for `format`.
    #[serde(default)]
    pub pattern: Option<String>,
    /// Require uniqueness across the run (`format` + `unique`).
    #[serde(default)]
    pub unique: bool,
    /// Plugin comparator name for `plugin`.
    #[serde(default)]
    pub plugin: Option<String>,
}

impl FieldRule {
    pub fn exact() -> Self {
        Self { comparator: ComparatorKind::Exact, tolerance: None, relative: false, pattern: None, unique: false, plugin: None }
    }
    pub fn tolerance_f64(&self) -> Option<f64> {
        match &self.tolerance {
            Some(serde_yaml::Value::Number(n)) => n.as_f64(),
            Some(serde_yaml::Value::String(s)) => s.parse::<f64>().ok(),
            _ => None,
        }
    }
    pub fn tolerance_str(&self) -> Option<String> {
        match &self.tolerance {
            Some(serde_yaml::Value::String(s)) => Some(s.clone()),
            Some(serde_yaml::Value::Number(n)) => Some(format!("{}s", n)),
            _ => None,
        }
    }
}

/// Data-driven comparison policy for one business output shape (design §11, §7).
///
/// ```yaml
/// name: loan-early-repayment
/// default: exact
/// fields:
///   amount:         { comparator: exact }
///   timestamp:      { comparator: tolerance, tolerance: 2s }
///   transaction_id: { comparator: format, pattern: "^TX-[0-9A-F]{12}$", unique: true }
///   events:         { comparator: unordered }
///   trace_id:       { comparator: ignore }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComparatorSpec {
    pub name: String,
    #[serde(default = "default_kind")]
    pub default: ComparatorKind,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldRule>,
}

fn default_kind() -> ComparatorKind {
    ComparatorKind::Exact
}

impl ComparatorSpec {
    pub fn exact(name: &str) -> Self {
        Self { name: name.to_string(), default: ComparatorKind::Exact, fields: BTreeMap::new() }
    }

    pub fn from_yaml(text: &str) -> Result<Self, ComparatorError> {
        let spec: ComparatorSpec = serde_yaml::from_str(text)?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), ComparatorError> {
        for (path, rule) in &self.fields {
            match rule.comparator {
                ComparatorKind::Format if rule.pattern.is_none() => {
                    return Err(ComparatorError::Spec(format!("{path}: format comparator needs `pattern`")))
                }
                ComparatorKind::Plugin if rule.plugin.is_none() => {
                    return Err(ComparatorError::Spec(format!("{path}: plugin comparator needs `plugin`")))
                }
                ComparatorKind::Numeric | ComparatorKind::Tolerance if rule.tolerance.is_none() => {
                    return Err(ComparatorError::Spec(format!("{path}: comparator needs `tolerance`")))
                }
                _ => {}
            }
            if let Some(p) = &rule.pattern {
                regex::Regex::new(p).map_err(|e| ComparatorError::Spec(format!("{path}: bad pattern: {e}")))?;
            }
        }
        Ok(())
    }

    /// Find the most specific rule for a JSON path (`a.b[2].c` is normalised to `a.b.*.c`).
    pub fn rule_for(&self, path: &str) -> Option<&FieldRule> {
        let segs = normalise(path);
        let mut best: Option<(&FieldRule, usize)> = None;
        for (pattern, rule) in &self.fields {
            let psegs = normalise(pattern);
            if matches(&psegs, &segs) {
                let specificity = psegs.iter().filter(|s| *s != "*").count() * 2 + psegs.len();
                if best.map(|(_, s)| specificity > s).unwrap_or(true) {
                    best = Some((rule, specificity));
                }
            }
        }
        best.map(|(r, _)| r)
    }
}

fn normalise(path: &str) -> Vec<String> {
    path.replace('[', ".").replace(']', "")
        .split('.')
        .filter(|s| !s.is_empty())
        .map(|s| if s.chars().all(|c| c.is_ascii_digit()) { "*".to_string() } else { s.to_string() })
        .collect()
}

fn matches(pattern: &[String], path: &[String]) -> bool {
    if pattern.len() != path.len() {
        return false;
    }
    pattern.iter().zip(path).all(|(p, s)| p == "*" || p == s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_resolves_paths() {
        let yaml = r#"
name: t
fields:
  amount: { comparator: exact }
  events: { comparator: unordered }
  events.*.ts: { comparator: tolerance, tolerance: 2s }
  trace_id: { comparator: ignore }
"#;
        let spec = ComparatorSpec::from_yaml(yaml).unwrap();
        assert_eq!(spec.rule_for("events[3].ts").unwrap().comparator, ComparatorKind::Tolerance);
        assert_eq!(spec.rule_for("trace_id").unwrap().comparator, ComparatorKind::Ignore);
        assert!(spec.rule_for("other").is_none());
    }
}
