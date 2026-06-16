use crate::model::{Dimensions,
                   RNNParams,
                   RNNState};
use linalg::{Matrix,
             Vector};
use nn_core::tanh_derivative;

/// 파라미터와 같은 형태의 그래디언트.
#[derive(Debug, Clone)]
pub struct RNNGradients {
    pub dwxh: Matrix,
    pub dwhh: Matrix,
    pub dwhy: Matrix,
    pub dbh: Vector,
    pub dby: Vector,
}

impl RNNGradients {
    fn zeros(dims: Dimensions) -> Self {
        Self {
            dwxh: Matrix::zeros(dims.hidden, dims.input),
            dwhh: Matrix::zeros(dims.hidden, dims.hidden),
            dwhy: Matrix::zeros(dims.output, dims.hidden),
            dbh: Vector::zeros(dims.hidden),
            dby: Vector::zeros(dims.output),
        }
    }

    fn add(&self, other: &RNNGradients) -> RNNGradients {
        RNNGradients {
            dwxh: self.dwxh.add(&other.dwxh),
            dwhh: self.dwhh.add(&other.dwhh),
            dwhy: self.dwhy.add(&other.dwhy),
            dbh: self.dbh.add(&other.dbh),
            dby: self.dby.add(&other.dby),
        }
    }
}

/// 한 시점의 그래디언트와, 이전 시점으로 전파할 은닉 상태
/// 그래디언트(`dh_next`).
struct StepGradients {
    grad: RNNGradients,
    dh_prev: Vector,
}

/// 한 시점의 역전파(BPTT 한 스텝).
///
/// - `h_t`: 현재 시점의 은닉 출력 (`tanh` 적용 후)
/// - `h_prev`: **이전 시점**의 은닉 출력 (`dwhh`에 필요)
/// - `dh_next`: 미래 시점에서 전파되어 온 은닉 상태 그래디언트
fn compute_step_gradients(
    params: &RNNParams,
    input: &Vector,
    target: &Vector,
    output: &Vector,
    h_t: &Vector,
    h_prev: &Vector,
    dh_next: &Vector,
) -> StepGradients {
    // softmax + 교차 엔트로피 ⇒ dy = output - target
    let dy = output.sub(target);

    // 은닉 상태에 대한 그래디언트: dh = W_hy^T · dy + dh_next
    let dh = params.why.transpose().vec_mul(&dy).add(dh_next);

    // tanh를 거슬러: dh_raw = dh ⊙ (1 - h_t²)
    let dh_raw = dh.hadamard(&tanh_derivative(h_t));

    let grad = RNNGradients {
        dwxh: dh_raw.outer(input),
        dwhh: dh_raw.outer(h_prev), // ← 이전 은닉 상태 h_{t-1} 사용 (BPTT 정의)
        dwhy: dy.outer(h_t),
        dbh: dh_raw.clone(),
        dby: dy,
    };

    // 이전 시점으로 전파: dh_prev = W_hh^T · dh_raw
    let dh_prev = params.whh.transpose().vec_mul(&dh_raw);

    StepGradients {
        grad,
        dh_prev,
    }
}

/// 시퀀스 전체에 대한 BPTT. 시점을 역순으로 훑으며 그래디언트를 누적한다.
///
/// `init_state`는 시점 0의 이전 은닉 상태(`h_{-1}`)로 쓰인다.
pub fn compute_gradients(
    params: &RNNParams,
    init_state: &RNNState,
    inputs: &[Vector],
    targets: &[Vector],
    outputs: &[Vector],
    states: &[RNNState],
) -> RNNGradients {
    let dims = params.dimensions();
    let mut accum = RNNGradients::zeros(dims);
    let mut dh_next = Vector::zeros(dims.hidden);

    for t in (0 .. inputs.len()).rev() {
        let h_t = &states[t].hidden;
        let h_prev = if t == 0 { &init_state.hidden } else { &states[t - 1].hidden };

        let step = compute_step_gradients(params, &inputs[t], &targets[t], &outputs[t], h_t, h_prev, &dh_next);
        accum = accum.add(&step.grad);
        dh_next = step.dh_prev;
    }

    accum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{forward::rnn_forward,
                model::{Dimensions,
                        init_hidden_state,
                        init_rnn}};
    use nn_core::cross_entropy_loss;
    use rand::{SeedableRng,
               rngs::StdRng};

    const DIMS: Dimensions = Dimensions {
        input: 2,
        hidden: 3,
        output: 2,
    };

    fn fixture() -> (RNNParams, RNNState, Vec<Vector>, Vec<Vector>) {
        let mut rng = StdRng::seed_from_u64(42);
        let params = init_rnn(DIMS, &mut rng);
        let init_state = init_hidden_state(DIMS.hidden);
        let inputs = vec![Vector::new(vec![1.0, 0.0]), Vector::new(vec![0.0, 1.0]), Vector::new(vec![1.0, 1.0])];
        let targets = vec![Vector::new(vec![0.0, 1.0]), Vector::new(vec![1.0, 0.0]), Vector::new(vec![0.0, 1.0])];
        (params, init_state, inputs, targets)
    }

    fn total_loss(params: &RNNParams, init_state: &RNNState, inputs: &[Vector], targets: &[Vector]) -> f64 {
        let (outputs, _) = rnn_forward(params, init_state, inputs);
        outputs.iter().zip(targets).map(|(o, t)| cross_entropy_loss(t, o)).sum()
    }

    fn analytic(params: &RNNParams, init_state: &RNNState, inputs: &[Vector], targets: &[Vector]) -> RNNGradients {
        let (outputs, states) = rnn_forward(params, init_state, inputs);
        compute_gradients(params, init_state, inputs, targets, &outputs, &states)
    }

    /// 한 파라미터 성분을 ±eps 흔들어 중심차분으로 수치 그래디언트를 구한다.
    fn numeric_grad(init_state: &RNNState, inputs: &[Vector], targets: &[Vector], base: &RNNParams, bump: impl Fn(&RNNParams, f64) -> RNNParams) -> f64 {
        const EPS: f64 = 1e-5;
        let plus = total_loss(&bump(base, EPS), init_state, inputs, targets);
        let minus = total_loss(&bump(base, -EPS), init_state, inputs, targets);
        (plus - minus) / (2.0 * EPS)
    }

    fn bump_matrix(m: &Matrix, i: usize, j: usize, eps: f64) -> Matrix {
        Matrix::from_fn(m.rows(), m.cols(), |r, c| m.get(r, c) + if r == i && c == j { eps } else { 0.0 })
    }

    fn bump_vector(v: &Vector, i: usize, eps: f64) -> Vector { Vector::from_fn(v.len(), |k| v[k] + if k == i { eps } else { 0.0 }) }

    #[test]
    fn analytic_gradients_match_finite_differences() {
        let (params, init_state, inputs, targets) = fixture();
        let g = analytic(&params, &init_state, &inputs, &targets);
        const TOL: f64 = 1e-6;

        // 모든 가중치 행렬을 성분별로 검증 (dwhh 가 핵심: h_{t-1} 사용)
        let matrices: [(&Matrix, &Matrix, &str); 3] = [(&g.dwxh, &params.wxh, "dwxh"), (&g.dwhh, &params.whh, "dwhh"), (&g.dwhy, &params.why, "dwhy")];
        for (grad, weight, name) in matrices {
            for i in 0 .. weight.rows() {
                for j in 0 .. weight.cols() {
                    let num = numeric_grad(&init_state, &inputs, &targets, &params, |p, eps| {
                        let mut q = p.clone();
                        match name {
                            | "dwxh" => q.wxh = bump_matrix(&p.wxh, i, j, eps),
                            | "dwhh" => q.whh = bump_matrix(&p.whh, i, j, eps),
                            | _ => q.why = bump_matrix(&p.why, i, j, eps),
                        }
                        q
                    });
                    assert!((grad.get(i, j) - num).abs() < TOL, "{name}[{i}][{j}] 해석={} 수치={num}", grad.get(i, j));
                }
            }
        }

        // 편향도 검증
        for i in 0 .. params.bh.len() {
            let num = numeric_grad(&init_state, &inputs, &targets, &params, |p, eps| {
                let mut q = p.clone();
                q.bh = bump_vector(&p.bh, i, eps);
                q
            });
            assert!((g.dbh[i] - num).abs() < TOL, "dbh[{i}] 해석={} 수치={num}", g.dbh[i]);
        }
        for i in 0 .. params.by.len() {
            let num = numeric_grad(&init_state, &inputs, &targets, &params, |p, eps| {
                let mut q = p.clone();
                q.by = bump_vector(&p.by, i, eps);
                q
            });
            assert!((g.dby[i] - num).abs() < TOL, "dby[{i}] 해석={} 수치={num}", g.dby[i]);
        }
    }
}
