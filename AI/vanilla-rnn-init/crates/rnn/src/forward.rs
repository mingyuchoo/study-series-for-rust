use crate::model::{RNNParams,
                   RNNState};
use linalg::Vector;
use nn_core::{softmax,
              tanh};

/// 한 시점의 순전파.
///
/// 반환: `(출력 분포 y_t, 다음 은닉 상태)`.
pub fn rnn_step(params: &RNNParams, state: &RNNState, input: &Vector) -> (Vector, RNNState) {
    // h_raw = W_xh · x + W_hh · h_prev + b_h
    let h_raw = params.wxh.vec_mul(input).add(&params.whh.vec_mul(&state.hidden)).add(&params.bh);
    let h_t = tanh(&h_raw);

    // y_raw = W_hy · h_t + b_y
    let y_raw = params.why.vec_mul(&h_t).add(&params.by);
    let y_t = softmax(&y_raw);

    (
        y_t,
        RNNState {
            hidden: h_t,
        },
    )
}

/// 시퀀스 전체에 대한 순전파.
///
/// 반환: `(시점별 출력, 시점별 은닉 상태)`. `states[t]`는 입력 `t` 처리 후의
/// 상태다.
pub fn rnn_forward(params: &RNNParams, init_state: &RNNState, inputs: &[Vector]) -> (Vec<Vector>, Vec<RNNState>) {
    let mut outputs = Vec::with_capacity(inputs.len());
    let mut states = Vec::with_capacity(inputs.len());
    let mut state = init_state.clone();

    for x in inputs {
        let (y, next) = rnn_step(params, &state, x);
        outputs.push(y);
        states.push(next.clone());
        state = next;
    }

    (outputs, states)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dimensions,
                       init_hidden_state,
                       init_rnn};
    use rand::{SeedableRng,
               rngs::StdRng};

    fn sample_inputs() -> Vec<Vector> { vec![Vector::new(vec![1.0, 0.0]), Vector::new(vec![0.0, 1.0]), Vector::new(vec![1.0, 1.0])] }

    #[test]
    fn outputs_are_probability_distributions() {
        let dims = Dimensions {
            input: 2,
            hidden: 4,
            output: 3,
        };
        let mut rng = StdRng::seed_from_u64(7);
        let params = init_rnn(dims, &mut rng);
        let (outputs, states) = rnn_forward(&params, &init_hidden_state(dims.hidden), &sample_inputs());

        assert_eq!(outputs.len(), 3);
        assert_eq!(states.len(), 3);
        for y in &outputs {
            assert_eq!(y.len(), dims.output);
            let sum: f64 = y.iter().sum();
            assert!((sum - 1.0).abs() < 1e-12);
            assert!(y.iter().all(|&p| (0.0 ..= 1.0).contains(&p)));
        }
    }

    #[test]
    fn hidden_state_stays_in_tanh_range() {
        let dims = Dimensions {
            input: 2,
            hidden: 4,
            output: 3,
        };
        let mut rng = StdRng::seed_from_u64(8);
        let params = init_rnn(dims, &mut rng);
        let (_, states) = rnn_forward(&params, &init_hidden_state(dims.hidden), &sample_inputs());
        for s in &states {
            assert!(s.hidden.iter().all(|&h| (-1.0 ..= 1.0).contains(&h)));
        }
    }
}
