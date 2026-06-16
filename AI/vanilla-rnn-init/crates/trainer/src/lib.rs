//! 학습 오케스트레이션.
//!
//! 순전파 → 손실 → 역전파 → 파라미터 갱신 루프를 돈다. 콘솔 출력을 직접 하지
//! 않고 [`ProgressObserver`]에 위임하여 학습 로직과 I/O를 분리한다.

use linalg::Vector;
use nn_core::cross_entropy_loss;
use rnn::{RNNParams,
          compute_gradients,
          init_hidden_state,
          rnn_forward,
          update_params};

/// 학습 하이퍼파라미터. 차원은 모델(`RNNParams`)이 이미 알고 있으므로 두지
/// 않는다.
#[derive(Debug, Clone, Copy)]
pub struct TrainConfig {
    pub learning_rate: f64,
    pub epochs: usize,
}

/// 에폭마다의 진행 상황을 받는 관찰자. 출력/로깅 방식을 호출자가 결정한다.
pub trait ProgressObserver {
    fn on_epoch(&mut self, epoch: usize, loss: f64);
}

/// 아무것도 하지 않는 관찰자 (테스트·무음 학습용).
pub struct SilentObserver;

impl ProgressObserver for SilentObserver {
    fn on_epoch(&mut self, _epoch: usize, _loss: f64) {}
}

/// 시퀀스를 `config.epochs`만큼 학습하고 갱신된 파라미터를 반환한다.
pub fn train(config: &TrainConfig, mut params: RNNParams, inputs: &[Vector], targets: &[Vector], observer: &mut impl ProgressObserver) -> RNNParams {
    let hidden_size = params.dimensions().hidden;

    for epoch in 1 ..= config.epochs {
        let init_state = init_hidden_state(hidden_size);
        let (outputs, states) = rnn_forward(&params, &init_state, inputs);

        let loss: f64 = outputs.iter().zip(targets).map(|(o, t)| cross_entropy_loss(t, o)).sum();

        let grads = compute_gradients(&params, &init_state, inputs, targets, &outputs, &states);
        params = update_params(config.learning_rate, &params, &grads);

        observer.on_epoch(epoch, loss);
    }

    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng,
               rngs::StdRng};
    use rnn::{Dimensions,
              init_rnn};

    /// 에폭별 손실을 모두 기록하는 관찰자.
    #[derive(Default)]
    struct RecordingObserver {
        losses: Vec<f64>,
    }

    impl ProgressObserver for RecordingObserver {
        fn on_epoch(&mut self, _epoch: usize, loss: f64) { self.losses.push(loss); }
    }

    fn fixture() -> (RNNParams, Vec<Vector>, Vec<Vector>) {
        let dims = Dimensions {
            input: 3,
            hidden: 5,
            output: 3,
        };
        let mut rng = StdRng::seed_from_u64(123);
        let params = init_rnn(dims, &mut rng);
        let inputs = vec![
            Vector::new(vec![1.0, 0.0, 0.0]),
            Vector::new(vec![0.0, 1.0, 0.0]),
            Vector::new(vec![0.0, 0.0, 1.0]),
        ];
        let targets = vec![
            Vector::new(vec![0.0, 1.0, 0.0]),
            Vector::new(vec![0.0, 0.0, 1.0]),
            Vector::new(vec![1.0, 0.0, 0.0]),
        ];
        (params, inputs, targets)
    }

    #[test]
    fn observer_called_once_per_epoch() {
        let (params, inputs, targets) = fixture();
        let config = TrainConfig {
            learning_rate: 0.1,
            epochs: 25,
        };
        let mut obs = RecordingObserver::default();
        train(&config, params, &inputs, &targets, &mut obs);
        assert_eq!(obs.losses.len(), 25);
    }

    #[test]
    fn training_reduces_loss() {
        let (params, inputs, targets) = fixture();
        let config = TrainConfig {
            learning_rate: 0.1,
            epochs: 200,
        };
        let mut obs = RecordingObserver::default();
        train(&config, params, &inputs, &targets, &mut obs);

        let first = obs.losses.first().copied().unwrap();
        let last = obs.losses.last().copied().unwrap();
        assert!(last < first, "손실이 줄지 않음: {first} -> {last}");
        assert!(last < 0.1, "최종 손실이 충분히 작지 않음: {last}");
    }

    #[test]
    fn zero_epochs_is_a_noop() {
        let (params, inputs, targets) = fixture();
        let config = TrainConfig {
            learning_rate: 0.1,
            epochs: 0,
        };
        let mut obs = SilentObserver;
        // 패닉 없이 그대로 반환되어야 한다
        let _ = train(&config, params, &inputs, &targets, &mut obs);
    }
}
