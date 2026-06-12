//! 실적 기록 패널·측정값 스테퍼·기록 잔디 문구.

use super::{Lang,
            pick};

impl Lang {
    // ── 실적 기록 ────────────────────────────────────────────
    pub fn records_heading(self) -> &'static str { pick(self, "실적 기록", "Records") }

    pub fn agg_auto_hint(self, aggregation_label: &str) -> String {
        match self {
            | Self::Ko => format!("{aggregation_label}(으)로 현재값에 자동 집계됩니다."),
            | Self::En => format!("Aggregated into the current value via {aggregation_label}."),
        }
    }

    pub fn record_popover_hint(self, aggregation_label: &str) -> String {
        match self {
            | Self::Ko => format!("측정값을 입력하면 {aggregation_label}(으)로 현재값에 바로 반영됩니다."),
            | Self::En => format!("Enter a value — it updates the current value via {aggregation_label}."),
        }
    }

    // ── 측정값 스테퍼 ────────────────────────────────────────
    pub fn step_increase(self) -> &'static str { pick(self, "측정값 늘리기", "Increase value") }

    pub fn step_decrease(self) -> &'static str { pick(self, "측정값 줄이기", "Decrease value") }

    pub fn step_size_aria(self) -> &'static str { pick(self, "한 클릭의 변화량 선택", "Select the step size") }

    pub fn note_placeholder(self) -> &'static str {
        pick(
            self,
            "오늘의 메모 (선택) — 느낌, 배운 점, 컨디션 등을 일기처럼 남겨 보세요.",
            "Today's note (optional) — how it went, what you learned, like a journal.",
        )
    }

    pub fn cmd_enter_hint(self) -> &'static str { pick(self, "⌘+Enter로도 기록", "⌘+Enter to record") }

    pub fn no_records_yet_panel(self) -> &'static str {
        pick(self, "아직 기록이 없습니다. 첫 실적을 기록해 보세요.", "No records yet — add the first one.")
    }

    pub fn recorded_toast(self, title: &str, amount: &str, unit: &str) -> String {
        let message = match self {
            | Self::Ko => format!("\"{title}\" {amount} {unit} 기록됨"),
            | Self::En => format!("Recorded {amount} {unit} for \"{title}\""),
        };
        message.replace("  ", " ").trim_end().to_string()
    }

    pub fn err_record(self, error: &str) -> String {
        match self {
            | Self::Ko => format!("실적 기록에 실패했습니다: {error}"),
            | Self::En => format!("Failed to record: {error}"),
        }
    }

    pub fn err_undo(self, error: &str) -> String {
        match self {
            | Self::Ko => format!("실행 취소에 실패했습니다: {error}"),
            | Self::En => format!("Failed to undo: {error}"),
        }
    }

    pub fn err_load_records(self, error: &str) -> String {
        match self {
            | Self::Ko => format!("실적 기록을 불러오지 못했습니다: {error}"),
            | Self::En => format!("Failed to load records: {error}"),
        }
    }

    pub fn err_add_record(self, error: &str) -> String {
        match self {
            | Self::Ko => format!("실적 기록 추가에 실패했습니다: {error}"),
            | Self::En => format!("Failed to add the record: {error}"),
        }
    }

    pub fn err_delete_record(self, error: &str) -> String {
        match self {
            | Self::Ko => format!("실적 기록 삭제에 실패했습니다: {error}"),
            | Self::En => format!("Failed to delete the record: {error}"),
        }
    }

    // ── 기록 잔디 ────────────────────────────────────────────
    pub fn grass_title(self) -> &'static str { pick(self, "기록 잔디", "Activity") }

    pub fn grass_range(self, scope: Option<&str>, weeks: i64) -> String {
        match (self, scope) {
            | (Self::Ko, Some(scope)) => format!("{scope} · 최근 {weeks}주"),
            | (Self::Ko, None) => format!("최근 {weeks}주"),
            | (Self::En, Some(scope)) => format!("{scope} · last {weeks} weeks"),
            | (Self::En, None) => format!("Last {weeks} weeks"),
        }
    }

    pub fn grass_no_record(self, date: &str) -> String {
        match self {
            | Self::Ko => format!("{date} · 기록 없음"),
            | Self::En => format!("{date} · no records"),
        }
    }

    pub fn grass_count(self, date: &str, count: u32) -> String {
        match self {
            | Self::Ko => format!("{date} · {count}건 기록"),
            | Self::En if count == 1 => format!("{date} · 1 record"),
            | Self::En => format!("{date} · {count} records"),
        }
    }

    pub fn grass_stats(self, recorded_days: usize, longest_streak: i64, this_week_days: usize) -> String {
        match self {
            | Self::Ko => format!("{recorded_days}일 기록 · 최장 연속 {longest_streak}일 · 이번 주 {this_week_days}일"),
            | Self::En => format!("{recorded_days} days recorded · longest streak {longest_streak} · this week {this_week_days}"),
        }
    }

    pub fn grass_less(self) -> &'static str { pick(self, "적음", "Less") }

    pub fn grass_more(self) -> &'static str { pick(self, "많음", "More") }
}
