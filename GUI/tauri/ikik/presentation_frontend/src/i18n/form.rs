//! 항목 등록·수정 폼과 빠른 추가 문구.

use super::{Lang,
            pick};

impl Lang {
    // ── 폼 ───────────────────────────────────────────────────
    pub fn form_edit_title(self) -> &'static str { pick(self, "항목 수정", "Edit item") }

    pub fn form_new_title(self) -> &'static str { pick(self, "새 IKIK 항목", "New IKIK item") }

    pub fn form_new_kind_title(self, kind_label: &str) -> String {
        match self {
            | Self::Ko => format!("새 {kind_label} 항목"),
            | Self::En => format!("New {kind_label} item"),
        }
    }

    pub fn err_title_required(self) -> &'static str { pick(self, "제목을 입력하세요.", "Enter a title.") }

    pub fn err_parent_required(self) -> &'static str { pick(self, "상위 항목을 선택하세요.", "Select a parent item.") }

    pub fn err_must_be_number(self, label: &str) -> String {
        match self {
            | Self::Ko => format!("{label}은(는) 숫자로 입력하세요."),
            | Self::En => format!("{label} must be a number."),
        }
    }

    pub fn breadcrumb_aria(self) -> &'static str { pick(self, "상위 항목 경로", "Parent item path") }

    pub fn goto_detail(self, kind_label: &str) -> String {
        match self {
            | Self::Ko => format!("{kind_label} 상세로 이동"),
            | Self::En => format!("Open {kind_label} detail"),
        }
    }

    pub fn stage_select_aria(self) -> &'static str { pick(self, "단계 선택", "Select a stage") }

    pub fn parent_select_placeholder(self) -> &'static str { pick(self, "상위 항목 선택", "Select a parent item") }

    pub fn unit_placeholder(self) -> &'static str { pick(self, "원, %, 건...", "%, hours, $...") }

    pub fn current_value_hint(self) -> &'static str {
        pick(
            self,
            "현재값은 실적 기록을 추가하면 집계 방식에 따라 자동 계산됩니다.",
            "The current value is aggregated automatically as you add records.",
        )
    }

    pub fn due_date_hint(self) -> &'static str { pick(self, "비워 두면 마감 없는 항목이 됩니다.", "Leave empty for an item without a deadline.") }

    pub fn target_date_hint(self) -> &'static str {
        pick(
            self,
            "목표값을 이 날짜까지 달성하는 것이 목표입니다.",
            "Aim to reach the target value by this date.",
        )
    }

    pub fn aggregation_select_aria(self) -> &'static str { pick(self, "집계 방식 선택", "Select an aggregation") }

    pub fn status_select_aria(self) -> &'static str { pick(self, "상태 선택", "Select a status") }

    // ── 빠른 추가 ────────────────────────────────────────────
    pub fn quick_add_placeholder(self, kind_label: &str) -> String {
        match self {
            | Self::Ko => format!("{kind_label} 제목을 입력하고 Enter"),
            | Self::En => format!("Type a {kind_label} title and press Enter"),
        }
    }

    pub fn more_details(self) -> &'static str { pick(self, "자세히 입력", "More details") }
}
