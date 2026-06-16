//! 진입점: 하이퍼파라미터를 조립하고 학습을 실행한 뒤 결과를 출력한다.
//!
//! 라이브러리 crate(linalg/nn-core/rnn/trainer)는 I/O를 모른다. 콘솔 출력은
//! 오직 이 바이너리에만 존재한다.

use linalg::Vector;
use rand::{SeedableRng,
           rngs::StdRng};
use rnn::{Dimensions,
          init_hidden_state,
          init_rnn,
          rnn_forward};
use trainer::{ProgressObserver,
              TrainConfig,
              train};

/// 10 에폭마다 손실을 콘솔에 출력하는 관찰자.
struct ConsoleObserver {
    every: usize,
}

impl ProgressObserver for ConsoleObserver {
    fn on_epoch(&mut self, epoch: usize, loss: f64) {
        if epoch.is_multiple_of(self.every) {
            println!("Epoch {epoch}, Loss: {loss:.6}");
        }
    }
}

fn main() {
    let dims = Dimensions {
        input: 3,
        hidden: 5,
        output: 3,
    };
    let config = TrainConfig {
        learning_rate: 0.1,
        epochs: 500,
    };

    let mut rng = StdRng::from_os_rng();
    let params = init_rnn(dims, &mut rng);

    // 간단한 순환 시퀀스: e1 -> e2 -> e3 -> e1
    let inputs: Vec<Vector> = vec![
        Vector::new(vec![1.0, 0.0, 0.0]),
        Vector::new(vec![0.0, 1.0, 0.0]),
        Vector::new(vec![0.0, 0.0, 1.0]),
    ];
    let targets: Vec<Vector> = vec![
        Vector::new(vec![0.0, 1.0, 0.0]),
        Vector::new(vec![0.0, 0.0, 1.0]),
        Vector::new(vec![1.0, 0.0, 0.0]),
    ];

    println!("RNN 학습 시작...");
    let mut observer = ConsoleObserver {
        every: 10,
    };
    let params = train(&config, params, &inputs, &targets, &mut observer);

    println!("\n학습된 모델 테스트:");
    let (outputs, _) = rnn_forward(&params, &init_hidden_state(dims.hidden), &inputs);
    for ((input, output), target) in inputs.iter().zip(&outputs).zip(&targets) {
        println!("입력: {:?}", input.as_slice());
        println!("예측: {:?}", output.as_slice());
        println!("정답: {:?}", target.as_slice());
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rnn::init_hidden_state;
    use trainer::SilentObserver;

    /// 바이너리 crate에서 전체 스택(init → train → forward)이 통합 동작하는지
    /// 확인.
    #[test]
    fn end_to_end_pipeline_runs_and_predicts() {
        let dims = Dimensions {
            input: 3,
            hidden: 5,
            output: 3,
        };
        let config = TrainConfig {
            learning_rate: 0.1,
            epochs: 300,
        };
        let mut rng = StdRng::seed_from_u64(2024);
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

        let params = train(&config, params, &inputs, &targets, &mut SilentObserver);
        let (outputs, _) = rnn_forward(&params, &init_hidden_state(dims.hidden), &inputs);

        // 각 출력은 확률 분포이고, argmax 가 정답과 일치해야 한다
        for (output, target) in outputs.iter().zip(&targets) {
            let sum: f64 = output.iter().sum();
            assert!((sum - 1.0).abs() < 1e-9);
            let pred = argmax(output.as_slice());
            let truth = argmax(target.as_slice());
            assert_eq!(pred, truth, "예측 클래스 불일치");
        }
    }

    fn argmax(xs: &[f64]) -> usize { xs.iter().enumerate().fold(0, |best, (i, &x)| if x > xs[best] { i } else { best }) }
}
