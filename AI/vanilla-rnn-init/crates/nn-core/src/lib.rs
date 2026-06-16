//! 신경망 공통 요소: 활성화 함수와 손실 함수.
//!
//! RNN에 한정되지 않는 재사용 가능한 원시 연산만 둔다. `linalg`에만 의존한다.

mod activation;
mod loss;

pub use activation::{softmax,
                     tanh,
                     tanh_derivative};
pub use loss::cross_entropy_loss;
