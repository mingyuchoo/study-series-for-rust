use domain::DomainError;
use serde::Serialize;

/// Discriminates the kind of failure so the frontend can react differently
/// (e.g. show validation errors inline vs. surface a generic database failure).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorKind {
    NotFound,
    Validation,
    Database,
    Internal,
}

/// Serializable error returned from Tauri commands.
///
/// Serializes as `{ "kind": "...", "message": "..." }`. Unlike a bare `String`
/// this preserves the error category and never leaks raw database error text to
/// the frontend.
#[derive(Debug, Clone, Serialize)]
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

    /// A malformed identifier supplied by the caller (e.g. an invalid UUID).
    pub fn invalid_id(message: impl Into<String>) -> Self { Self::new(ApiErrorKind::Validation, message) }
}

impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        match error {
            | DomainError::ContactNotFound => Self::new(ApiErrorKind::NotFound, "연락처를 찾을 수 없습니다."),
            | DomainError::InvalidContactData(message) => Self::new(ApiErrorKind::Validation, message),
            // Raw DB error text is intentionally not forwarded to the frontend, but
            // it is logged server-side so failures remain diagnosable.
            | DomainError::DatabaseError(detail) => {
                tracing::error!(error = %detail, "database error");
                Self::new(ApiErrorKind::Database, "데이터베이스 오류가 발생했습니다.")
            },
            | DomainError::InternalError(message) => {
                tracing::error!(error = %message, "internal error");
                Self::new(ApiErrorKind::Internal, message)
            },
        }
    }
}
