//! 바닐라 RNN 도메인 로직 (순수 라이브러리).
//!
//! 하이퍼파라미터·콘솔 출력을 전혀 모른다. 입력을 받아 순전파/역전파/파라미터
//! 갱신을 수행하는 순수 함수들만 노출한다.
//!
//! 계층: `linalg` + `nn-core`에 의존.

mod backward;
mod forward;
mod model;
mod optimizer;

pub use backward::{RNNGradients,
                   compute_gradients};
pub use forward::{rnn_forward,
                  rnn_step};
pub use model::{Dimensions,
                RNNParams,
                RNNState,
                init_hidden_state,
                init_rnn};
pub use optimizer::update_params;
