//! 민감정보 마스킹.
//!
//! 두 가지 방식이 함께 걸립니다.
//!
//! - **필드 이름 기준** — `rrn`, `account`, `password` 같은 이름의 필드는 값
//!   전체를 가립니다. 도구 명세(`Spec.mask_fields`)와 정책
//!   의무(`mask_fields`)가 목록을 넓힐 수 있습니다.
//! - **패턴 기준** — 필드 이름과 무관하게 **모든 문자열 잎사귀**에 정규식을
//!   적용합니다. 주민번호가 `note` 필드 안 문장에 섞여 있어도 잡아야 하기
//!   때문입니다.

use regex::Regex;
use serde_json::{Map,
                 Value};
use std::{collections::HashSet,
          sync::LazyLock};

// 적용 순서가 중요합니다 — 아래 scrub_string 참고.
static RE_RRN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{6}-\d{7}\b").unwrap());
static RE_CARD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{4}-\d{4}-\d{4}-\d{4}\b").unwrap());
static RE_PHONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b01[016789]-?\d{3,4}-?\d{4}\b").unwrap());
static RE_EMAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").unwrap());
// 마지막 그룹이 4~8자리인 것은 `2026-07-10` 같은 날짜와 충돌하지 않게 하기
// 위함입니다.
static RE_ACCOUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{2,6}-\d{2,6}-\d{4,8}\b").unwrap());

/// 이름만으로 민감하다고 보는 기본 필드들 (대소문자 무시).
const DEFAULT_SENSITIVE_FIELDS: &[&str] = &[
    "rrn",
    "ssn",
    "resident_registration_number",
    "account",
    "account_no",
    "account_number",
    "card",
    "card_no",
    "password",
    "secret",
];

/// 민감정보 마스커.
#[derive(Debug, Clone)]
pub struct Masker {
    field_names: HashSet<String>,
}

impl Default for Masker {
    fn default() -> Self { Self::new() }
}

impl Masker {
    pub fn new() -> Self {
        Self {
            field_names: DEFAULT_SENSITIVE_FIELDS.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// 값을 마스킹한 **복사본**을 돌려줍니다. 입력은 건드리지 않습니다.
    ///
    /// `extra_fields` 는 도구 명세와 정책 의무가 추가로 지정한 필드 이름입니다.
    pub fn mask(&self, value: &Value, extra_fields: &[String]) -> Value {
        let extra: HashSet<String> = extra_fields.iter().map(|s| s.to_lowercase()).collect();
        self.mask_value(value, &extra)
    }

    fn mask_value(&self, value: &Value, extra: &HashSet<String>) -> Value {
        match value {
            | Value::Object(map) => {
                let mut out = Map::new();
                for (key, v) in map {
                    let lower = key.to_lowercase();
                    if self.field_names.contains(&lower) || extra.contains(&lower) {
                        // 이름이 걸리면 **하위 트리로 내려가지 않고** 값 전체를 가립니다.
                        out.insert(key.clone(), mask_field_value(v));
                    } else {
                        out.insert(key.clone(), self.mask_value(v, extra));
                    }
                }
                Value::Object(out)
            },
            | Value::Array(items) => Value::Array(items.iter().map(|v| self.mask_value(v, extra)).collect()),
            // 필드 이름에 걸리지 않은 문자열도 패턴 검사는 무조건 받습니다.
            | Value::String(s) => Value::String(scrub_string(s)),
            | other => other.clone(),
        }
    }
}

/// 이름으로 걸린 필드의 값을 가립니다.
fn mask_field_value(v: &Value) -> Value {
    match v {
        | Value::String(s) => Value::String(partial_mask(s)),
        | _ => Value::String("***MASKED***".to_string()),
    }
}

/// 문자열 안에 섞인 민감정보를 패턴으로 가립니다.
///
/// **적용 순서가 결과를 바꿉니다.** 전화번호를 계좌번호보다 먼저 처리해야
/// 합니다 — `010-1234-5678` 은 계좌번호 정규식(`\d{2,6}-\d{2,6}-\d{4,8}`)에도
/// 걸리므로, 순서가 뒤바뀌면 전화번호가 계좌번호로 오분류되어 접두사 보존이
/// 깨집니다.
fn scrub_string(s: &str) -> String {
    let out = RE_RRN.replace_all(s, "******-*******");
    let out = RE_CARD.replace_all(&out, "****-****-****-****");
    let out = RE_PHONE.replace_all(&out, |c: &regex::Captures| partial_mask(&c[0]));
    let out = RE_ACCOUNT.replace_all(&out, "***-***-****");
    let out = RE_EMAIL.replace_all(&out, |c: &regex::Captures| mask_email(&c[0]));
    out.into_owned()
}

/// 앞 몇 글자만 남기고 가립니다.
fn partial_mask(s: &str) -> String {
    let runes: Vec<char> = s.chars().collect();
    if runes.len() <= 2 {
        return "***".to_string();
    }
    let keep = if runes.len() > 8 { 3 } else { 2 };
    let mut out: String = runes[.. keep].iter().collect();
    out.push_str(&"*".repeat(runes.len() - keep));
    out
}

/// 이메일은 도메인을 남깁니다 — 존재는 알려도 되는 정보이기 때문입니다.
fn mask_email(email: &str) -> String {
    let Some(at) = email.find('@') else {
        return "***".to_string();
    };
    if at == 0 {
        return "***".to_string();
    }
    let local: Vec<char> = email[.. at].chars().collect();
    let domain = &email[at ..]; // '@' 포함
    if local.len() <= 1 {
        return format!("*{domain}");
    }
    let mut out = String::new();
    out.push(local[0]);
    out.push_str(&"*".repeat(local.len() - 1));
    out.push_str(domain);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn masks_rrn_and_card_wholesale() {
        let m = Masker::new();
        let out = m.mask(&json!({"note": "주민번호 900101-1234567 입니다"}), &[]);
        assert_eq!(out["note"], json!("주민번호 ******-******* 입니다"));
    }

    #[test]
    fn phone_is_masked_before_account_pattern() {
        // 순서가 뒤바뀌면 전화번호가 계좌번호로 오분류됩니다.
        let m = Masker::new();
        let out = m.mask(&json!({"note": "010-1234-5678"}), &[]);
        assert_eq!(out["note"], json!("010**********"));
    }

    #[test]
    fn email_keeps_domain() {
        let m = Masker::new();
        let out = m.mask(&json!({"note": "hong@example.com"}), &[]);
        assert_eq!(out["note"], json!("h***@example.com"));
    }

    #[test]
    fn masks_by_field_name_even_when_pattern_does_not_match() {
        let m = Masker::new();
        let out = m.mask(&json!({"password": "hunter2xyz"}), &[]);
        assert_eq!(out["password"], json!("hun*******"));
    }

    #[test]
    fn extra_fields_widen_the_set() {
        let m = Masker::new();
        let out = m.mask(&json!({"phone": "unlisted-value"}), &["phone".into()]);
        assert_eq!(out["phone"], json!("unl***********"));
    }

    #[test]
    fn field_name_match_does_not_recurse_into_subtree() {
        let m = Masker::new();
        let out = m.mask(&json!({"account": {"deep": "x"}}), &[]);
        assert_eq!(out["account"], json!("***MASKED***"));
    }

    #[test]
    fn recurses_into_arrays_and_nested_objects() {
        let m = Masker::new();
        let out = m.mask(&json!({"rows": [{"note": "900101-1234567"}]}), &[]);
        assert_eq!(out["rows"][0]["note"], json!("******-*******"));
    }

    #[test]
    fn input_is_not_mutated() {
        let m = Masker::new();
        let input = json!({"note": "900101-1234567"});
        let _ = m.mask(&input, &[]);
        assert_eq!(input["note"], json!("900101-1234567"));
    }

    #[test]
    fn non_strings_pass_through() {
        let m = Masker::new();
        let out = m.mask(&json!({"amount": 1200000, "ok": true}), &[]);
        assert_eq!(out["amount"], json!(1200000));
        assert_eq!(out["ok"], json!(true));
    }

    #[test]
    fn dates_are_not_mistaken_for_account_numbers() {
        // 계좌 정규식의 마지막 그룹이 4자리 이상이라 `2026-07-10` 은 걸리지 않습니다.
        let m = Masker::new();
        let out = m.mask(&json!({"signed_at": "2026-07-10"}), &[]);
        assert_eq!(out["signed_at"], json!("2026-07-10"));
    }
}
