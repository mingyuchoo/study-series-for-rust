//! PII tokenisation / masking applied before anything reaches an LLM (stack §17).
use regex::Regex;
use std::sync::OnceLock;

struct Rules(Vec<(Regex, &'static str)>);

fn rules() -> &'static Rules {
    static R: OnceLock<Rules> = OnceLock::new();
    R.get_or_init(|| {
        Rules(vec![
            (Regex::new(r"\b\d{6}-[1-4]\d{6}\b").unwrap(), "[RRN]"),
            (Regex::new(r"\b(?:\d[ -]?){13,19}\b").unwrap(), "[CARD]"),
            (
                Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(),
                "[EMAIL]",
            ),
            (
                Regex::new(r"\b01[016789]-?\d{3,4}-?\d{4}\b").unwrap(),
                "[PHONE]",
            ),
            (Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap(), "[SSN]"),
            (
                Regex::new(r"(?i)\b(?:sk-ant-|sk-|AKIA)[A-Za-z0-9_\-]{8,}").unwrap(),
                "[SECRET]",
            ),
        ])
    })
}

/// Mask personally identifiable information / secrets. Deterministic and idempotent.
pub fn mask(text: &str) -> String {
    let mut out = text.to_string();
    for (re, repl) in &rules().0 {
        out = re.replace_all(&out, *repl).into_owned();
    }
    out
}

pub fn contains_pii(text: &str) -> bool {
    rules().0.iter().any(|(re, _)| re.is_match(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn masks_common_pii() {
        let s = "cust 900101-1234567 card 4111 1111 1111 1111 mail a.b@example.com key sk-ant-abcdefghijkl";
        let m = mask(s);
        assert!(
            m.contains("[RRN]")
                && m.contains("[CARD]")
                && m.contains("[EMAIL]")
                && m.contains("[SECRET]")
        );
        assert!(!contains_pii(&m));
    }
}
