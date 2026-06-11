//! 실적 기록 스테퍼의 스텝 칩 구성. 한 클릭의 변화량을 지표 스스로
//! 정하는 단일 결정이라 화면이 아닌 여기에 둔다.
//!
//! 규칙: 목표값의 크기가 클수록 큰 칩을 더하고, 합계형이 아닌 작은
//! 지표(체지방률, 수면 시간 등)에는 소수 칩(±0.1)을 더한다. 칩이
//! ±1 하나뿐이면 화면은 칩 줄을 그리지 않는다(순수 스테퍼).

use contracts::KpiAggregation;

/// 지표에 어울리는 스텝 후보(오름차순, 1~3개).
pub fn step_chips(target_value: Option<f64>, aggregation: KpiAggregation) -> Vec<f64> {
    let target = target_value.unwrap_or(0.0).abs();
    let mut chips = Vec::new();
    // 합계형은 증분 기록이라 정수 단위가 자연스럽다. 수준 지표는
    // 목표가 작을 때 0.1 단위가 의미를 가진다(15.2% 등).
    if aggregation != KpiAggregation::Sum && target < 30.0 {
        chips.push(0.1);
    }
    chips.push(1.0);
    if target >= 30.0 {
        chips.push(10.0);
    }
    if target >= 300.0 {
        chips.push(100.0);
    }
    chips
}

/// 기본 선택 칩: 소수 지표는 ±0.1, 큰 지표는 ±10, 그 외 ±1.
pub fn default_step(chips: &[f64]) -> f64 {
    if chips.contains(&0.1) {
        0.1
    } else if chips.contains(&10.0) {
        10.0
    } else {
        1.0
    }
}

/// 한 번의 클릭. 0.1 스텝의 부동소수 오차가 쌓이지 않도록 소수
/// 첫째 자리로 반올림하고, 음수 기록은 만들지 않는다.
pub fn bump_value(value: f64, step: f64, direction: f64) -> f64 { (((value + direction * step) * 10.0).round() / 10.0).max(0.0) }

#[cfg(test)]
mod tests {
    use super::*;
    use KpiAggregation::{Average,
                         Latest,
                         Sum};

    #[test]
    fn small_level_metrics_get_decimal_chips() {
        // 체지방률 15%, 수면 7시간, 배당 수익률 5%
        assert_eq!(step_chips(Some(15.0), Latest), vec![0.1, 1.0]);
        assert_eq!(step_chips(Some(7.0), Average), vec![0.1, 1.0]);
        assert_eq!(default_step(&[0.1, 1.0]), 0.1);
    }

    #[test]
    fn small_sum_metrics_stay_a_pure_stepper() {
        // 주간 운동 3회, 월 신규 강의 2개 — 칩 한 개면 칩 줄을 숨긴다.
        assert_eq!(step_chips(Some(3.0), Sum), vec![1.0]);
        assert_eq!(default_step(&[1.0]), 1.0);
    }

    #[test]
    fn medium_metrics_add_ten_chip() {
        // 임대 순수익 200만원, 완강률 60%, 수강생 100명
        assert_eq!(step_chips(Some(200.0), Average), vec![1.0, 10.0]);
        assert_eq!(step_chips(Some(100.0), Sum), vec![1.0, 10.0]);
        assert_eq!(default_step(&[1.0, 10.0]), 10.0);
    }

    #[test]
    fn large_metrics_add_hundred_chip() {
        // 계약 단가 1000만원, 3대 중량 300kg
        assert_eq!(step_chips(Some(1000.0), Average), vec![1.0, 10.0, 100.0]);
        assert_eq!(step_chips(Some(300.0), Latest), vec![1.0, 10.0, 100.0]);
        assert_eq!(default_step(&[1.0, 10.0, 100.0]), 10.0);
    }

    #[test]
    fn missing_target_falls_back_to_small_metric_rules() {
        assert_eq!(step_chips(None, Latest), vec![0.1, 1.0]);
        assert_eq!(step_chips(None, Sum), vec![1.0]);
    }

    #[test]
    fn bump_rounds_decimals_and_clamps_at_zero() {
        assert_eq!(bump_value(15.2, 0.1, 1.0), 15.3);
        assert_eq!(bump_value(0.3, 0.1, -1.0), 0.2);
        // 0.1 + 0.2 != 0.30000000000000004 방어
        assert_eq!(bump_value(bump_value(0.1, 0.1, 1.0), 0.1, 1.0), 0.3);
        assert_eq!(bump_value(0.0, 10.0, -1.0), 0.0);
        assert_eq!(bump_value(5.0, 10.0, -1.0), 0.0);
    }
}
