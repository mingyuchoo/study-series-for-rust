//! 도메인 레이어의 에러 타입.
//!
//! 도메인은 영속화 기술(rusqlite 등)을 알지 못하므로, 백엔드에서 발생한 에러는
//! [`RepositoryError::Backend`]에 메시지 문자열로 담아 추상화한다.

use thiserror::Error;

/// 엔티티 불변식(invariant)을 위반한 입력에 대한 검증 에러.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationError {
    /// 이름이 비어 있다.
    #[error("name must not be empty")]
    EmptyName,

    /// 이메일 형식이 올바르지 않다.
    #[error("invalid email: {0}")]
    InvalidEmail(String),

    /// 전화번호가 비어 있다.
    #[error("phone must not be empty")]
    EmptyPhone,
}

/// 저장소(영속화) 동작에서 발생하는 에러.
///
/// 도메인은 구체적인 백엔드를 모르므로 백엔드 고유 에러는
/// [`RepositoryError::Backend`]에 문자열로 담는다. `Clone` 을 derive 하는
/// 이유는 GUI 메시지(`Message: Clone`)로 운반되기 때문이다.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepositoryError {
    /// 갱신/삭제처럼 식별자가 필요한 동작인데 `id` 가 없다.
    #[error("operation requires a persisted address with an id")]
    MissingId,

    /// 백엔드(데이터베이스 등)에서 발생한 에러.
    #[error("storage backend error: {0}")]
    Backend(String),
}
