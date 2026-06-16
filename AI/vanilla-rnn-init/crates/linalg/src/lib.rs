//! 순수 선형대수 레이어 (의존성 0).
//!
//! `Vector`/`Matrix` 뉴타입으로 차원 불변식을 한곳에 가둔다.
//! 모든 연산은 차원이 맞지 않으면 조용히 절삭하는 대신 `panic`으로 실패한다.

mod matrix;
mod vector;

pub use matrix::Matrix;
pub use vector::Vector;
