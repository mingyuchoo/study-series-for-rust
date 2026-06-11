//! 화면 표시용 공통 포맷터. 숫자·시각 표기는 화면마다 달라지면 안 되는
//! 단일 결정이므로 여기에만 정의한다.

/// 소수부가 없으면 정수로(`60`), 있으면 그대로(`18.2`) 보여 준다.
pub fn format_value(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

/// RFC3339 시각(UTC)에서 분 단위까지만 잘라 "YYYY-MM-DD HH:MM"으로
/// 보여 준다.
pub fn format_timestamp(timestamp: &str) -> String { timestamp.chars().take(16).map(|ch| if ch == 'T' { ' ' } else { ch }).collect() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integral_values_drop_the_fraction() {
        assert_eq!(format_value(60.0), "60");
        assert_eq!(format_value(0.0), "0");
        assert_eq!(format_value(-3.0), "-3");
    }

    #[test]
    fn fractional_values_keep_their_digits() {
        assert_eq!(format_value(18.2), "18.2");
        assert_eq!(format_value(0.5), "0.5");
    }

    #[test]
    fn timestamps_truncate_to_minutes() {
        assert_eq!(format_timestamp("2026-06-11T04:16:33.123Z"), "2026-06-11 04:16");
        assert_eq!(format_timestamp("2026-06-11"), "2026-06-11");
    }
}
