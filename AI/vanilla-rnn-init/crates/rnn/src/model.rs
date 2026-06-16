use linalg::{Matrix,
             Vector};
use rand::{Rng,
           rngs::StdRng};

/// RNN의 차원 정보. 모델 내부 표현을 캐묻지 않고 차원을 질의할 수 있게 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub input: usize,
    pub hidden: usize,
    pub output: usize,
}

/// 학습 가능한 파라미터.
#[derive(Debug, Clone)]
pub struct RNNParams {
    pub wxh: Matrix, // input  -> hidden
    pub whh: Matrix, // hidden -> hidden (recurrent)
    pub why: Matrix, // hidden -> output
    pub bh: Vector,  // hidden bias
    pub by: Vector,  // output bias
}

impl RNNParams {
    /// 파라미터로부터 차원을 도출한다.
    pub fn dimensions(&self) -> Dimensions {
        Dimensions {
            input: self.wxh.cols(),
            hidden: self.bh.len(),
            output: self.by.len(),
        }
    }
}

/// 시점 사이로 전달되는 은닉 상태.
#[derive(Debug, Clone)]
pub struct RNNState {
    pub hidden: Vector,
}

const INIT_SCALE: f64 = 0.01;

fn random_matrix(rows: usize, cols: usize, rng: &mut StdRng) -> Matrix { Matrix::from_fn(rows, cols, |_, _| rng.random_range(-1.0 ..= 1.0) * INIT_SCALE) }

/// 작은 난수 가중치와 0 편향으로 파라미터를 초기화한다.
pub fn init_rnn(dims: Dimensions, rng: &mut StdRng) -> RNNParams {
    RNNParams {
        wxh: random_matrix(dims.hidden, dims.input, rng),
        whh: random_matrix(dims.hidden, dims.hidden, rng),
        why: random_matrix(dims.output, dims.hidden, rng),
        bh: Vector::zeros(dims.hidden),
        by: Vector::zeros(dims.output),
    }
}

/// 영(0) 은닉 상태.
pub fn init_hidden_state(hidden_size: usize) -> RNNState {
    RNNState {
        hidden: Vector::zeros(hidden_size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn init_produces_expected_shapes() {
        let dims = Dimensions {
            input: 4,
            hidden: 6,
            output: 3,
        };
        let mut rng = StdRng::seed_from_u64(0);
        let p = init_rnn(dims, &mut rng);

        assert_eq!((p.wxh.rows(), p.wxh.cols()), (6, 4));
        assert_eq!((p.whh.rows(), p.whh.cols()), (6, 6));
        assert_eq!((p.why.rows(), p.why.cols()), (3, 6));
        assert_eq!(p.bh.len(), 6);
        assert_eq!(p.by.len(), 3);
    }

    #[test]
    fn dimensions_round_trips() {
        let dims = Dimensions {
            input: 4,
            hidden: 6,
            output: 3,
        };
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(init_rnn(dims, &mut rng).dimensions(), dims);
    }
}
