//! 출력 정규화.
//!
//! 어댑터가 돌려준 값을 **순수 JSON**(맵·배열·스칼라)으로 만듭니다.
//!
//! 이 단계가 없으면 아래 단계들(의무 집행·PII 마스킹)의 타입 분기가 조용히
//! 건너뛰는 경우가 생깁니다 — 그리고 **그 결과는 조용한 정보 노출입니다.**
//! 마스킹되어야 할 값이 마스킹되지 않은 채 LLM 에게 나갑니다.
//!
//! `serde_json::Value` 는 이미 순수 JSON 이므로 대부분 항등 변환이지만,
//! 어댑터가 NaN 이나 무한대 같은 표현 불가능한 수를 냈을 때를 정리합니다.

use serde_json::{Map,
                 Value};

pub(crate) fn normalize(v: &Value) -> Value {
    match v {
        | Value::Object(m) => {
            let mut out = Map::new();
            for (k, val) in m {
                out.insert(k.clone(), normalize(val));
            }
            Value::Object(out)
        },
        | Value::Array(items) => Value::Array(items.iter().map(normalize).collect()),
        | Value::Number(n) => {
            // JSON 으로 표현할 수 없는 수는 null 로 둡니다 — 그대로 두면 직렬화가
            // 실패하거나 스키마 검증이 엉뚱한 오류를 냅니다.
            if n.as_f64().map(|f| f.is_finite()).unwrap_or(true) {
                Value::Number(n.clone())
            } else {
                Value::Null
            }
        },
        | other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_json_passes_through_unchanged() {
        let v = json!({"a": 1, "b": [1, "x", true, null], "c": {"d": 2.5}});
        assert_eq!(normalize(&v), v);
    }

    #[test]
    fn nested_structures_are_walked() {
        let v = json!({"rows":[{"inner":{"deep":[1,2]}}]});
        assert_eq!(normalize(&v), v);
    }

    #[test]
    fn strings_are_never_treated_as_sequences() {
        // 문자열을 배열로 오인해 글자 단위로 분해하면 마스킹이 무의미해집니다.
        let v = json!({"note": "900101-1234567"});
        assert_eq!(normalize(&v), v);
    }
}
