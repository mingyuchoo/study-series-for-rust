// 검증 이유는 와이어 계약(contracts)이 단일 정의를 갖고, 도메인이
// 재노출한다 — 응용 계층은 domain만 의존하면 된다.
pub use contracts::ValidationIssue;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("IKIK item not found")]
    ItemNotFound,

    /// 검증 실패. 구조화된 이유가 와이어까지 그대로 전달되어
    /// 프런트엔드가 현재 언어로 문구를 조립한다.
    #[error("{}", .0.default_message())]
    Validation(ValidationIssue),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}
