//! 정책 의무 집행 — 출력을 좁힙니다.
//!
//! "재무 데이터는 요약만 가능하고 원본 다운로드 불가", "고객 개인정보는 마스킹
//! 후 LLM에 전달" 같은 정책은 **호출을 막는 것이 아니라 결과를 좁히는
//! 것**입니다. 허용/거부 2분법으로는 표현할 수 없습니다.
//!
//! 순서가 중요합니다: **출력 스키마 검증 뒤, 마스킹 앞.**
//!
//! - 스키마 검증보다 뒤여야 레거시의 계약 위반과 정책의 축소가 헷갈리지
//!   않습니다.
//! - 마스킹보다 앞이어야 제거된 필드가 "가려졌지만 존재함"으로 새어나가지
//!   않습니다.

use crate::policy::Obligations;
use serde_json::{Map,
                 Value};

/// 의무를 적용합니다. 반환: `(shaped, narrowed)`.
pub(crate) fn apply(out: &Map<String, Value>, o: &Obligations) -> (Map<String, Value>, bool) {
    if o.is_zero() {
        return (out.clone(), false);
    }
    let redact: Vec<String> = o.redact_fields.iter().map(|s| s.to_lowercase()).collect();
    let (shaped, redacted) = redact_value(&Value::Object(out.clone()), &redact);

    let mut m = shaped.as_object().cloned().unwrap_or_default();
    let mut truncated = false;
    if o.max_rows > 0 {
        truncated = truncate_arrays(&mut m, o.max_rows as usize);
    }
    (m, redacted || truncated)
}

/// 필드를 **제거합니다** (가리는 것이 아니라). 중첩과 배열을 따라 내려갑니다.
fn redact_value(v: &Value, fields: &[String]) -> (Value, bool) {
    match v {
        | Value::Object(map) => {
            let mut out = Map::new();
            let mut hit = false;
            for (k, val) in map {
                if fields.contains(&k.to_lowercase()) {
                    hit = true;
                    continue; // 값이 아예 없습니다.
                }
                let (child, h) = redact_value(val, fields);
                hit |= h;
                out.insert(k.clone(), child);
            }
            (Value::Object(out), hit)
        },
        | Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            let mut hit = false;
            for i in items {
                let (child, h) = redact_value(i, fields);
                hit |= h;
                out.push(child);
            }
            (Value::Array(out), hit)
        },
        | other => (other.clone(), false),
    }
}

/// **최상위 배열 필드만** 자릅니다.
///
/// 행 제한은 "몇 건을 보여줄 것인가"에 대한 규칙이므로 중첩 배열까지 파고들지
/// 않습니다.
///
/// 잘라낸 결과에는 `truncated: true` 가 붙습니다 — LLM 이 "전부 조회했다"고
/// 착각한 채 요약하면, 사용자는 없는 사실을 없다고 믿게 됩니다.
fn truncate_arrays(out: &mut Map<String, Value>, max: usize) -> bool {
    let mut truncated = false;
    for (_, v) in out.iter_mut() {
        if let Value::Array(items) = v
            && items.len() > max
        {
            items.truncate(max);
            truncated = true;
        }
    }
    if truncated {
        out.insert("truncated".into(), Value::Bool(true));
        out.insert("truncated_reason".into(), Value::String("정책이 반환 행 수를 제한했습니다".into()));
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> { v.as_object().unwrap().clone() }

    #[test]
    fn zero_obligations_change_nothing() {
        let out = obj(json!({"a": 1}));
        let (shaped, narrowed) = apply(&out, &Obligations::default());
        assert_eq!(shaped, out);
        assert!(!narrowed);
    }

    #[test]
    fn redacted_fields_are_removed_not_masked() {
        // 마스킹은 "값이 있지만 가려짐", 제거는 "값 자체가 없음" 입니다.
        let out = obj(json!({"contracts":[{"title":"계약","signed_at":"2026-01-15"}]}));
        let o = Obligations {
            redact_fields: vec!["signed_at".into()],
            ..Default::default()
        };
        let (shaped, narrowed) = apply(&out, &o);
        assert!(narrowed);
        let c = &shaped["contracts"][0];
        assert!(c.get("signed_at").is_none(), "제거되어야 할 필드가 남았습니다");
        assert_eq!(c["title"], json!("계약"));
    }

    #[test]
    fn redaction_is_case_insensitive_and_recursive() {
        let out = obj(json!({"nested":{"deep":{"RRN":"900101-1234567"}}}));
        let o = Obligations {
            redact_fields: vec!["rrn".into()],
            ..Default::default()
        };
        let (shaped, _) = apply(&out, &o);
        assert!(shaped["nested"]["deep"].get("RRN").is_none());
    }

    #[test]
    fn max_rows_truncates_and_says_so() {
        // 잘렸다는 사실을 알리지 않으면 LLM 이 "전부 조회했다"고 착각합니다.
        let out = obj(json!({"contracts":[1,2,3,4,5]}));
        let o = Obligations {
            max_rows: 3,
            ..Default::default()
        };
        let (shaped, narrowed) = apply(&out, &o);
        assert!(narrowed);
        assert_eq!(shaped["contracts"].as_array().unwrap().len(), 3);
        assert_eq!(shaped["truncated"], json!(true));
        assert!(shaped["truncated_reason"].as_str().unwrap().contains("정책"));
    }

    #[test]
    fn no_truncation_marker_when_under_the_limit() {
        let out = obj(json!({"contracts":[1,2]}));
        let o = Obligations {
            max_rows: 3,
            ..Default::default()
        };
        let (shaped, narrowed) = apply(&out, &o);
        assert!(!narrowed);
        assert!(shaped.get("truncated").is_none());
    }

    #[test]
    fn nested_arrays_are_not_truncated() {
        // 행 제한은 최상위 결과 목록에 대한 규칙입니다.
        let out = obj(json!({"row":{"items":[1,2,3,4,5]}}));
        let o = Obligations {
            max_rows: 2,
            ..Default::default()
        };
        let (shaped, _) = apply(&out, &o);
        assert_eq!(shaped["row"]["items"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn redaction_and_truncation_compose() {
        let out = obj(json!({
            "contracts": [
                {"title":"a","signed_at":"2026-01-01"},
                {"title":"b","signed_at":"2026-02-01"},
                {"title":"c","signed_at":"2026-03-01"}
            ]
        }));
        let o = Obligations {
            redact_fields: vec!["signed_at".into()],
            max_rows: 1,
            ..Default::default()
        };
        let (shaped, narrowed) = apply(&out, &o);
        assert!(narrowed);
        assert_eq!(shaped["contracts"].as_array().unwrap().len(), 1);
        assert!(shaped["contracts"][0].get("signed_at").is_none());
    }
}
