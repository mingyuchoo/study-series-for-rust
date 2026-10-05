//! 애플리케이션(use case) 레이어의 에러 타입.

use domain::error::{RepositoryError,
                    ValidationError};
use thiserror::Error;

/// use case 실행 중 발생할 수 있는 에러를 한데 모은 타입.
///
/// 도메인 검증 에러와 저장소 에러를 `#[from]` 으로 받아 `?` 연산자로 자연스럽게
/// 전파된다. `Clone` 을 derive 하는 이유는 GUI 메시지(`Message: Clone`)로
/// 운반되기 때문이다.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AppError {
    /// 입력 검증 실패.
    #[error(transparent)]
    Validation(#[from] ValidationError),

    /// 저장소 동작 실패.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
