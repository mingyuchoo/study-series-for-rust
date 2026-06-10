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

/// Tauri 커맨드가 반환하는 직렬화 가능한 오류.
/// `{ "kind": "...", "message": "..." }` 형태로 직렬화된다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub message: String,
}

impl ApiError {
    pub fn new(kind: ApiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// 호출자가 보낸 식별자가 잘못된 경우(예: 유효하지 않은 UUID).
    pub fn invalid_id(message: impl Into<String>) -> Self { Self::new(ApiErrorKind::Validation, message) }
}
