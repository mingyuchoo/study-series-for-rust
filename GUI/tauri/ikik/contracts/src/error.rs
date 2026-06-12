use crate::ItemKind;
use serde::{Deserialize,
            Serialize};

/// 실패 종류를 구분해 프런트엔드가 다르게 반응할 수 있게 한다
/// (예: 검증 오류는 인라인 표시, 데이터베이스 오류는 일반 안내).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorKind {
    NotFound,
    Validation,
    Database,
    Internal,
}

/// 검증 실패의 구조화된 이유. 메시지 문자열 대신 이유와 파라미터가
/// 와이어로 내려가, 프런트엔드가 현재 언어로 문구를 조립한다.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ValidationIssue {
    TitleRequired,
    IdentityMustBeRoot,
    ParentRequired { kind: ItemKind },
    ParentKindMismatch { kind: ItemKind, parent_kind: ItemKind },
    SelfParent,
    KpiFieldsOnNonKpi,
    DueDateOnIdentity,
    MeasurementNotNumeric,
    MeasurementsRequireKpi,
}

impl ValidationIssue {
    /// 로그·구버전 클라이언트용 기본(한국어) 문구. 화면 표기는
    /// 프런트엔드가 현재 언어로 따로 조립한다.
    pub fn default_message(self) -> String {
        match self {
            | Self::TitleRequired => "제목을 입력하세요.".to_string(),
            | Self::IdentityMustBeRoot => "Identity는 최상위 항목이어야 합니다.".to_string(),
            | Self::ParentRequired {
                kind,
            } => format!("{} 항목의 상위 항목을 선택하세요.", kind.label()),
            | Self::ParentKindMismatch {
                kind,
                parent_kind,
            } => format!("{} 항목은 {} 아래에 둘 수 없습니다.", kind.label(), parent_kind.label()),
            | Self::SelfParent => "자기 자신을 상위 항목으로 선택할 수 없습니다.".to_string(),
            | Self::KpiFieldsOnNonKpi => "목표값, 현재값, 단위는 Key Performance Indicator 항목에서만 사용합니다.".to_string(),
            | Self::DueDateOnIdentity => "마감 기한은 Identity 항목에서는 사용할 수 없습니다.".to_string(),
            | Self::MeasurementNotNumeric => "측정값은 유효한 숫자여야 합니다.".to_string(),
            | Self::MeasurementsRequireKpi => "측정 기록은 Key Performance Indicator 항목에서만 사용할 수 있습니다.".to_string(),
        }
    }
}

/// Tauri 커맨드가 반환하는 직렬화 가능한 오류.
/// `{ "kind": "...", "message": "...", "issue": {...}? }` 형태로 직렬화된다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiError {
    pub kind: ApiErrorKind,
    /// 기본(한국어) 문구. `issue`가 있으면 프런트엔드는 그것으로 현재
    /// 언어의 문구를 다시 만든다.
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<ValidationIssue>,
}

impl ApiError {
    pub fn new(kind: ApiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            issue: None,
        }
    }

    /// 구조화된 검증 오류.
    pub fn validation(issue: ValidationIssue) -> Self {
        Self {
            kind: ApiErrorKind::Validation,
            message: issue.default_message(),
            issue: Some(issue),
        }
    }

    /// 호출자가 보낸 식별자가 잘못된 경우(예: 유효하지 않은 UUID).
    pub fn invalid_id(message: impl Into<String>) -> Self { Self::new(ApiErrorKind::Validation, message) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_issue_serializes_with_code_tag_and_params() {
        let issue = ValidationIssue::ParentRequired {
            kind: ItemKind::Kra,
        };
        assert_eq!(serde_json::to_string(&issue).unwrap(), r#"{"code":"parent_required","kind":"kra"}"#);

        let api_error = ApiError::validation(issue);
        let json = serde_json::to_string(&api_error).unwrap();
        let parsed: ApiError = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.issue, Some(issue));
    }

    #[test]
    fn api_error_without_issue_still_deserializes() {
        // 구버전 형태({kind, message})도 그대로 읽힌다.
        let parsed: ApiError = serde_json::from_str(r#"{"kind":"validation","message":"oops"}"#).unwrap();
        assert_eq!(parsed.issue, None);
    }
}
