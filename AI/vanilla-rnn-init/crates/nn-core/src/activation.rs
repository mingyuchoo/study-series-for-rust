use linalg::Vector;

/// 성분별 `tanh`.
pub fn tanh(v: &Vector) -> Vector { v.map(f64::tanh) }

/// `tanh` 출력 `y = tanh(x)`에 대한 미분 `1 - y²`.
///
/// 입력은 활성화 **출력값**(`tanh`를 이미 적용한 값)이어야 한다.
pub fn tanh_derivative(activated: &Vector) -> Vector { activated.map(|y| 1.0 - y * y) }

/// 수치 안정화(최댓값 빼기)를 적용한 softmax.
pub fn softmax(v: &Vector) -> Vector {
    let max_x = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps = v.map(|x| (x - max_x).exp());
    let sum: f64 = exps.iter().sum();
    exps.map(|e| e / sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tanh_is_odd_and_bounded() {
        let v = Vector::new(vec![0.0, 100.0, -100.0]);
        let t = tanh(&v);
        assert!((t[0] - 0.0).abs() < 1e-12);
        assert!((t[1] - 1.0).abs() < 1e-9);
        assert!((t[2] + 1.0).abs() < 1e-9);
    }

    #[test]
    fn tanh_derivative_from_output() {
        // y = 0 -> 1 - 0 = 1 ; y = 0.5 -> 1 - 0.25 = 0.75
        let y = Vector::new(vec![0.0, 0.5]);
        assert_eq!(tanh_derivative(&y).as_slice(), &[1.0, 0.75]);
    }

    #[test]
    fn softmax_sums_to_one() {
        let s = softmax(&Vector::new(vec![1.0, 2.0, 3.0]));
        let sum: f64 = s.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12);
        // 큰 입력일수록 큰 확률
        assert!(s[2] > s[1] && s[1] > s[0]);
    }

    #[test]
    fn softmax_is_numerically_stable() {
        // 매우 큰 값에도 오버플로(NaN/Inf) 없이 합이 1
        let s = softmax(&Vector::new(vec![1000.0, 1001.0, 1002.0]));
        let sum: f64 = s.iter().sum();
        assert!(s.iter().all(|x| x.is_finite()));
        assert!((sum - 1.0).abs() < 1e-12);
    }
}
