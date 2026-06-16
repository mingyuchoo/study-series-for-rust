use linalg::Vector;

/// 교차 엔트로피 손실 `-Σ tᵢ · ln(oᵢ)`.
///
/// `ln(0)` 방지를 위해 작은 `epsilon`을 더한다.
pub fn cross_entropy_loss(target: &Vector, output: &Vector) -> f64 {
    const EPS: f64 = 1e-8;
    -target.iter().zip(output.iter()).map(|(t, o)| t * (o + EPS).ln()).sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_prediction_is_near_zero() {
        let target = Vector::new(vec![0.0, 1.0, 0.0]);
        let output = Vector::new(vec![0.0, 1.0, 0.0]);
        assert!(cross_entropy_loss(&target, &output).abs() < 1e-6);
    }

    #[test]
    fn confident_wrong_prediction_is_large() {
        let target = Vector::new(vec![1.0, 0.0, 0.0]);
        let good = cross_entropy_loss(&target, &Vector::new(vec![0.9, 0.05, 0.05]));
        let bad = cross_entropy_loss(&target, &Vector::new(vec![0.05, 0.9, 0.05]));
        assert!(bad > good);
    }

    #[test]
    fn zero_probability_does_not_produce_infinity() {
        // epsilon 덕분에 ln(0) -> -inf 이 발생하지 않는다
        let target = Vector::new(vec![1.0, 0.0]);
        let output = Vector::new(vec![0.0, 1.0]);
        assert!(cross_entropy_loss(&target, &output).is_finite());
    }
}
