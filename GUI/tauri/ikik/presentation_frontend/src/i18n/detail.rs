//! 항목 상세 화면(변경 이력 포함) 문구.

use super::{Lang,
            pick};

impl Lang {
    // ── 상세 화면 ────────────────────────────────────────────
    pub fn no_description(self) -> &'static str { pick(self, "설명 없음", "No description") }

    pub fn agg_hint_percent(self, aggregation_label: &str, percent: i64) -> String {
        match self {
            | Self::Ko => format!("{aggregation_label} 집계 · {percent}% 달성"),
            | Self::En => format!("{aggregation_label} aggregation · {percent}% achieved"),
        }
    }

    pub fn agg_hint(self, aggregation_label: &str) -> String {
        match self {
            | Self::Ko => format!("{aggregation_label} 집계"),
            | Self::En => format!("{aggregation_label} aggregation"),
        }
    }

    pub fn no_records_yet_detail(self) -> &'static str {
        pick(
            self,
            "아직 기록이 없습니다. 아래에서 첫 실적을 기록해 보세요.",
            "No records yet — add the first one below.",
        )
    }

    pub fn sub_items(self) -> &'static str { pick(self, "하위 항목", "Sub-items") }

    pub fn no_sub_items(self) -> &'static str { pick(self, "하위 항목이 없습니다", "No sub-items") }

    pub fn change_history(self) -> &'static str { pick(self, "변경 이력", "Change history") }

    pub fn err_load_revisions(self, error: &str) -> String {
        match self {
            | Self::Ko => format!("변경 이력을 불러오지 못했습니다: {error}"),
            | Self::En => format!("Failed to load the change history: {error}"),
        }
    }

    pub fn item_created(self) -> &'static str { pick(self, "항목 생성", "Item created") }
}
