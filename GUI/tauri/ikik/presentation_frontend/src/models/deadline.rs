//! 마감 기한 칩 표기. 날짜와 D-day를 병기하는 단일 결정을 모든 화면이
//! 공유한다. Key Performance Indicator는 마감이 아니라 목표 달성일이므로
//! "~까지" 표기로 구분한다.

use crate::i18n::Lang;
use chrono::{Datelike,
             NaiveDate};
use contracts::ItemKind;

/// 임박(검정 반전 칩)으로 표시하는 남은 일수 상한.
const SOON_DAYS: i64 = 7;

/// 브라우저 로컬 시간대 기준 오늘 날짜.
pub fn local_today() -> Option<NaiveDate> {
    let now = js_sys::Date::new_0();
    NaiveDate::from_ymd_opt(now.get_full_year() as i32, now.get_month() + 1, now.get_date())
}

/// 마감 기한 칩 한 개. `class`는 CSS 보조 클래스(빈 문자열·soon·overdue).
#[derive(Debug, Clone, PartialEq)]
pub struct DueChip {
    pub class: &'static str,
    pub text: String,
}

/// 연도가 다르면 연도까지 붙여 "6월 30일" / "2027년 1월 5일"처럼 보여 준다.
fn format_due_date(date: NaiveDate, today: NaiveDate, lang: Lang) -> String {
    match lang {
        | Lang::Ko =>
            if date.year() == today.year() {
                format!("{}월 {}일", date.month(), date.day())
            } else {
                format!("{}년 {}월 {}일", date.year(), date.month(), date.day())
            },
        | Lang::En => {
            const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
            let month = MONTHS[date.month0() as usize];
            if date.year() == today.year() {
                format!("{} {}", month, date.day())
            } else {
                format!("{} {}, {}", month, date.day(), date.year())
            }
        },
    }
}

/// "YYYY-MM-DD" 마감 기한을 칩 표기로 바꾼다. Identity나 파싱 불가
/// 값에는 칩을 만들지 않는다.
pub fn due_chip(due_date: &str, kind: ItemKind, lang: Lang, today: NaiveDate) -> Option<DueChip> {
    if kind == ItemKind::Identity {
        return None;
    }
    let date = NaiveDate::parse_from_str(due_date, "%Y-%m-%d").ok()?;
    let days_left = (date - today).num_days();
    let class = if days_left < 0 {
        "overdue"
    } else if days_left <= SOON_DAYS {
        "soon"
    } else {
        ""
    };
    let date_text = format_due_date(date, today, lang);

    // Key Performance Indicator는 지표가 끝나는 날이 아니라 목표값을
    // 달성해야 하는 시점이므로 D-day 대신 "~까지"로 읽힌다.
    let text = if kind == ItemKind::Kpi {
        match lang {
            | Lang::Ko => format!("{date_text}까지"),
            | Lang::En => format!("by {date_text}"),
        }
    } else if days_left > 0 {
        format!("{date_text} · D-{days_left}")
    } else if days_left == 0 {
        format!("{date_text} · D-day")
    } else {
        match lang {
            | Lang::Ko => format!("{date_text} · {}일 지남", -days_left),
            | Lang::En => format!("{date_text} · {}d overdue", -days_left),
        }
    };

    Some(DueChip {
        class,
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, d).unwrap() }

    #[test]
    fn igt_chip_shows_date_with_dday() {
        let today = day(2026, 6, 12);
        let chip = due_chip("2026-06-15", ItemKind::Igt, Lang::Ko, today).unwrap();
        assert_eq!(chip.class, "soon");
        assert_eq!(chip.text, "6월 15일 · D-3");

        let chip = due_chip("2026-06-30", ItemKind::Kra, Lang::Ko, today).unwrap();
        assert_eq!(chip.class, "");
        assert_eq!(chip.text, "6월 30일 · D-18");

        let chip = due_chip("2026-06-12", ItemKind::Igt, Lang::En, today).unwrap();
        assert_eq!(chip.class, "soon");
        assert_eq!(chip.text, "Jun 12 · D-day");
    }

    #[test]
    fn overdue_chip_counts_days_past() {
        let today = day(2026, 6, 12);
        let chip = due_chip("2026-06-10", ItemKind::Igt, Lang::Ko, today).unwrap();
        assert_eq!(chip.class, "overdue");
        assert_eq!(chip.text, "6월 10일 · 2일 지남");

        let chip = due_chip("2026-06-10", ItemKind::Igt, Lang::En, today).unwrap();
        assert_eq!(chip.text, "Jun 10 · 2d overdue");
    }

    #[test]
    fn kpi_chip_uses_by_date_wording() {
        let today = day(2026, 6, 12);
        let chip = due_chip("2026-07-31", ItemKind::Kpi, Lang::Ko, today).unwrap();
        assert_eq!(chip.class, "");
        assert_eq!(chip.text, "7월 31일까지");

        let chip = due_chip("2026-07-31", ItemKind::Kpi, Lang::En, today).unwrap();
        assert_eq!(chip.text, "by Jul 31");
    }

    #[test]
    fn different_year_includes_year() {
        let today = day(2026, 6, 12);
        let chip = due_chip("2027-01-05", ItemKind::Kra, Lang::Ko, today).unwrap();
        assert!(chip.text.starts_with("2027년 1월 5일"));

        let chip = due_chip("2027-01-05", ItemKind::Kra, Lang::En, today).unwrap();
        assert!(chip.text.starts_with("Jan 5, 2027"));
    }

    #[test]
    fn identity_and_garbage_make_no_chip() {
        let today = day(2026, 6, 12);
        assert_eq!(due_chip("2026-06-15", ItemKind::Identity, Lang::Ko, today), None);
        assert_eq!(due_chip("not-a-date", ItemKind::Igt, Lang::Ko, today), None);
    }
}
