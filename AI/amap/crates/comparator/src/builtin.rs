//! Built-in deterministic comparators.
use crate::{Comparator, ComparisonContext};
use chrono::{DateTime, Utc};
use serde_json::Value;

pub struct ExactComparator;
impl Comparator for ExactComparator {
    fn name(&self) -> &str {
        "exact"
    }
    fn compare(&self, expected: &Value, actual: &Value, _: &ComparisonContext<'_>) -> Option<String> {
        if json_eq(expected, actual) {
            None
        } else {
            Some("values differ".to_string())
        }
    }
}

/// Numerically-aware equality (`1` == `1.0`), otherwise structural.
pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(p), Some(q)) => p == q,
            _ => x == y,
        },
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_eq(p, q)),
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map(|w| json_eq(v, w)).unwrap_or(false))
        }
        _ => a == b,
    }
}

pub struct NumericComparator;
impl Comparator for NumericComparator {
    fn name(&self) -> &str {
        "numeric"
    }
    fn compare(&self, expected: &Value, actual: &Value, ctx: &ComparisonContext<'_>) -> Option<String> {
        let (Some(e), Some(a)) = (as_f64(expected), as_f64(actual)) else {
            return Some("not numeric".into());
        };
        let tol = ctx.rule.tolerance_f64().unwrap_or(0.0);
        let limit = if ctx.rule.relative { tol * e.abs() } else { tol };
        if (e - a).abs() <= limit {
            None
        } else {
            Some(format!("|{e} - {a}| exceeds tolerance {limit}"))
        }
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Timestamp equality within a duration (`2s`, `500ms`, `1m`). Accepts RFC3339 strings or epoch numbers.
pub struct TimestampComparator;
impl Comparator for TimestampComparator {
    fn name(&self) -> &str {
        "tolerance"
    }
    fn compare(&self, expected: &Value, actual: &Value, ctx: &ComparisonContext<'_>) -> Option<String> {
        let tol_ms = ctx.rule.tolerance_str().and_then(|s| parse_duration_ms(&s)).unwrap_or(0);
        let (Some(e), Some(a)) = (to_epoch_ms(expected), to_epoch_ms(actual)) else {
            return Some("not a timestamp".into());
        };
        if (e - a).abs() <= tol_ms {
            None
        } else {
            Some(format!("timestamps differ by {}ms (> {}ms)", (e - a).abs(), tol_ms))
        }
    }
}

pub fn parse_duration_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    let (num, unit) = s.split_at(s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len()));
    let n: f64 = num.trim().parse().ok()?;
    let mult = match unit.trim() {
        "ms" => 1.0,
        "" | "s" => 1000.0,
        "m" => 60_000.0,
        "h" => 3_600_000.0,
        "d" => 86_400_000.0,
        _ => return None,
    };
    Some((n * mult) as i64)
}

fn to_epoch_ms(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_f64().map(|f| if f > 1e12 { f as i64 } else { (f * 1000.0) as i64 }),
        Value::String(s) => DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc).timestamp_millis())
            .ok()
            .or_else(|| s.parse::<f64>().ok().map(|f| (f * 1000.0) as i64)),
        _ => None,
    }
}

/// Value must match a regex; optionally unique across the run (`Transaction UUID: FORMAT + UNIQUE`).
pub struct FormatComparator;
impl Comparator for FormatComparator {
    fn name(&self) -> &str {
        "format"
    }
    fn compare(&self, _expected: &Value, actual: &Value, ctx: &ComparisonContext<'_>) -> Option<String> {
        let Some(pattern) = &ctx.rule.pattern else { return Some("format comparator without pattern".into()) };
        let re = match regex::Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => return Some(format!("bad pattern: {e}")),
        };
        let text = match actual {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if !re.is_match(&text) {
            return Some(format!("`{text}` does not match `{pattern}`"));
        }
        if ctx.rule.unique {
            let key = format!("{}={}", ctx.path, text);
            let mut seen = ctx.seen_unique.lock().unwrap();
            if !seen.insert(key) {
                return Some(format!("`{text}` is not unique"));
            }
        }
        None
    }
}

/// Order-insensitive array equality (multiset of canonical JSON).
pub struct UnorderedComparator;
impl Comparator for UnorderedComparator {
    fn name(&self) -> &str {
        "unordered"
    }
    fn compare(&self, expected: &Value, actual: &Value, _: &ComparisonContext<'_>) -> Option<String> {
        let (Value::Array(e), Value::Array(a)) = (expected, actual) else {
            return if json_eq(expected, actual) { None } else { Some("not arrays".into()) };
        };
        let mut ec: Vec<String> = e.iter().map(canonical).collect();
        let mut ac: Vec<String> = a.iter().map(canonical).collect();
        ec.sort();
        ac.sort();
        if ec == ac {
            None
        } else {
            Some(format!("array contents differ ({} vs {} items)", e.len(), a.len()))
        }
    }
}

pub fn canonical(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<_> = m.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys.iter().map(|k| format!("{}:{}", serde_json::to_string(k).unwrap(), canonical(&m[*k]))).collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(a) => format!("[{}]", a.iter().map(canonical).collect::<Vec<_>>().join(",")),
        Value::Number(n) => n.as_f64().map(|f| format!("{f}")).unwrap_or_else(|| n.to_string()),
        other => other.to_string(),
    }
}

pub struct IgnoreComparator;
impl Comparator for IgnoreComparator {
    fn name(&self) -> &str {
        "ignore"
    }
    fn compare(&self, _: &Value, _: &Value, _: &ComparisonContext<'_>) -> Option<String> {
        None
    }
}
