//! 도메인 오류 → 공유 `ApiError` 변환.
//!
//! `ApiError` 타입은 `contracts`가 정의한다. 두 타입 모두 외부 크레이트
//! 소속이라(orphan rule) `From` 대신 자유 함수로 변환한다.

pub use contracts::{ApiError,
                    ApiErrorKind};
use domain::DomainError;

pub fn domain_error_to_api(error: DomainError) -> ApiError {
    match error {
        | DomainError::ItemNotFound => ApiError::new(ApiErrorKind::NotFound, "IVKIK 항목을 찾을 수 없습니다."),
        | DomainError::InvalidIvkikData(message) => ApiError::new(ApiErrorKind::Validation, message),
        // Raw DB error text is intentionally not forwarded to the frontend, but
        // it is logged server-side so failures remain diagnosable.
        | DomainError::DatabaseError(detail) => {
            tracing::error!(error = %detail, "database error");
            ApiError::new(ApiErrorKind::Database, "데이터베이스 오류가 발생했습니다.")
        },
        | DomainError::InternalError(message) => {
            tracing::error!(error = %message, "internal error");
            ApiError::new(ApiErrorKind::Internal, message)
        },
    }
}
