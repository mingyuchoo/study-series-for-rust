//! JSON Schema Draft 2020-12 검증 (입력·출력).
//!
//! 입력 스키마는 LLM이 만든 인자를 통제하고, 출력 스키마는 **레거시 응답이 도구
//! 계약을 지키는지** 확인합니다. 출력 검증이 없으면 레거시가 계약을 어긴 응답을
//! 돌려줘도 게이트웨이가 그대로 흘려보내고, LLM은 그것을 사실로 받아들입니다.

use anyhow::{Context as _,
             Result,
             anyhow};
use serde_json::Value;

/// 스키마 문서를 컴파일합니다.
///
/// `format` 단언을 켭니다(주석이 아니라 검증). Go 의 `compiler.AssertFormat()`
/// 대응 — 켜지 않으면 `format: date-time` 이 장식이 되고 아무 문자열이나
/// 통과합니다.
pub fn compile(document: &Value) -> Result<jsonschema::Validator> {
    if document.is_null() {
        return Err(anyhow!("schema is required"));
    }
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .build(document)
        .map_err(|e| anyhow!("invalid JSON Schema: {e}"))
}

/// 값이 스키마를 만족하는지 검증합니다.
pub fn validate(value: &Value, document: &Value) -> Result<()> {
    let validator = compile(document).context("compile schema")?;
    if let Err(err) = validator.validate(value) {
        return Err(anyhow!("JSON Schema validation failed: {err}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn invoice_schema() -> Value {
        json!({
            "type": "object",
            "properties": { "invoice_id": { "type": "string" } },
            "required": ["invoice_id"],
            "additionalProperties": false,
        })
    }

    #[test]
    fn accepts_valid_input() {
        assert!(validate(&json!({"invoice_id": "INV-1"}), &invoice_schema()).is_ok());
    }

    #[test]
    fn rejects_missing_required_field() {
        assert!(validate(&json!({}), &invoice_schema()).is_err());
    }

    #[test]
    fn rejects_unknown_field() {
        // 닫힌 스키마 — LLM 이 지어낸 인자를 조용히 통과시키지 않습니다.
        let v = json!({"invoice_id": "INV-1", "sql": "DROP TABLE"});
        assert!(validate(&v, &invoice_schema()).is_err());
    }

    #[test]
    fn rejects_wrong_type() {
        assert!(validate(&json!({"invoice_id": 42}), &invoice_schema()).is_err());
    }

    #[test]
    fn nil_schema_is_rejected() {
        assert!(compile(&Value::Null).is_err());
    }

    #[test]
    fn format_is_asserted_not_annotated() {
        let s = json!({"type": "string", "format": "date-time"});
        assert!(validate(&json!("2026-07-10T09:00:00Z"), &s).is_ok());
        assert!(validate(&json!("not-a-date"), &s).is_err());
    }
}
