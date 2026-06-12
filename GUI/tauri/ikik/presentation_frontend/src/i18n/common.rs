//! 공통 동작·명사, 헤더, 삭제 확인, 보드 탭, 스토어 오류 문구.

use super::{Lang,
            pick};

impl Lang {
    // ── 공통 동작 ─────────────────────────────────────────────
    pub fn add(self) -> &'static str { pick(self, "추가", "Add") }

    pub fn cancel(self) -> &'static str { pick(self, "취소", "Cancel") }

    pub fn delete(self) -> &'static str { pick(self, "삭제", "Delete") }

    pub fn edit(self) -> &'static str { pick(self, "수정", "Edit") }

    pub fn detail(self) -> &'static str { pick(self, "상세", "Details") }

    pub fn save(self) -> &'static str { pick(self, "저장", "Save") }

    pub fn record(self) -> &'static str { pick(self, "기록", "Record") }

    pub fn back_to_list(self) -> &'static str { pick(self, "목록으로", "Back to list") }

    pub fn undo(self) -> &'static str { pick(self, "실행 취소", "Undo") }

    pub fn loading(self) -> &'static str { pick(self, "로딩 중...", "Loading...") }

    // ── 공통 명사(폼 라벨·표 헤더 공용) ───────────────────────
    pub fn title_label(self) -> &'static str { pick(self, "제목", "Title") }

    pub fn description_label(self) -> &'static str { pick(self, "설명", "Description") }

    pub fn status_field(self) -> &'static str { pick(self, "상태", "Status") }

    pub fn stage_label(self) -> &'static str { pick(self, "단계", "Stage") }

    pub fn parent_label(self) -> &'static str { pick(self, "상위 항목", "Parent item") }

    pub fn target_label(self) -> &'static str { pick(self, "목표값", "Target") }

    pub fn unit_label(self) -> &'static str { pick(self, "단위", "Unit") }

    pub fn aggregation_field(self) -> &'static str { pick(self, "집계 방식", "Aggregation") }

    /// Key Result Area·Income Generating Task의 마감 기한 라벨.
    pub fn due_date_label(self) -> &'static str { pick(self, "마감 기한", "Due date") }

    /// Key Performance Indicator의 목표 달성일 라벨. 지표가 끝나는 날이
    /// 아니라 목표값을 달성해야 하는 시점이라 마감과 표기를 구분한다.
    pub fn target_date_label(self) -> &'static str { pick(self, "목표 달성일", "Target date") }

    pub fn progress_label(self) -> &'static str { pick(self, "진행률", "Progress") }

    pub fn top_level(self) -> &'static str { pick(self, "최상위", "Top level") }

    pub fn missing_parent(self) -> &'static str { pick(self, "(연결된 상위 항목 없음)", "(missing parent)") }

    pub fn none_label(self) -> &'static str { pick(self, "없음", "None") }

    // ── 헤더 ─────────────────────────────────────────────────
    pub fn tagline(self) -> &'static str {
        pick(
            self,
            "정체성에서 핵심 성과 지표까지, 큰 그림을 실행과 피드백으로 연결합니다.",
            "From Identity to Key Performance Indicator — connect the big picture to action and feedback.",
        )
    }

    pub fn search(self) -> &'static str { pick(self, "검색", "Search") }

    pub fn search_placeholder(self) -> &'static str {
        pick(
            self,
            "정체성, 핵심 결과 영역, 소득 창출 업무, 핵심 성과 지표 검색...",
            "Search Identity, Key Result Area, Income Generating Task, Key Performance Indicator...",
        )
    }

    pub fn reset(self) -> &'static str { pick(self, "초기화", "Reset") }

    pub fn new_item(self) -> &'static str { pick(self, "새 항목", "New item") }

    pub fn to_light_theme(self) -> &'static str { pick(self, "라이트 테마로 전환", "Switch to light theme") }

    pub fn to_dark_theme(self) -> &'static str { pick(self, "다크 테마로 전환", "Switch to dark theme") }

    /// 전환될 언어를 버튼에 보여 준다: 한국어 화면에는 "EN", 영어 화면에는
    /// "한".
    pub fn lang_button(self) -> &'static str { pick(self, "EN", "한") }

    pub fn to_other_lang(self) -> &'static str { pick(self, "Switch to English", "한국어로 전환") }

    // ── 삭제 확인 ────────────────────────────────────────────
    pub fn confirm_delete_title(self) -> &'static str { pick(self, "항목 삭제", "Delete item") }

    pub fn confirm_delete_aria(self) -> &'static str { pick(self, "IKIK 항목 삭제 확인", "Confirm IKIK item deletion") }

    pub fn confirm_delete_body(self, title: &str) -> String {
        match self {
            | Self::Ko => format!("\"{title}\" 항목을 삭제할까요? 하위 항목도 함께 삭제됩니다."),
            | Self::En => format!("Delete \"{title}\"? Its sub-items will also be deleted."),
        }
    }

    pub fn item_not_found(self) -> &'static str { pick(self, "항목을 찾을 수 없습니다.", "Item not found.") }

    // ── 보드 탭 ──────────────────────────────────────────────
    pub fn tabs_aria(self) -> &'static str { pick(self, "IKIK 단계 탭", "IKIK stage tabs") }

    pub fn dashboard_tab(self) -> &'static str { pick(self, "대시보드", "Dashboard") }

    pub fn structure_tab(self) -> &'static str { pick(self, "전체 구조", "Structure") }

    pub fn no_search_results(self) -> &'static str { pick(self, "검색 결과가 없습니다.", "No search results.") }

    // ── 스토어 오류 ──────────────────────────────────────────
    pub fn err_load_items(self) -> &'static str { pick(self, "IKIK 항목을 불러오지 못했습니다", "Failed to load IKIK items") }

    pub fn err_search(self) -> &'static str { pick(self, "검색에 실패했습니다", "Search failed") }

    pub fn err_refresh_list(self) -> &'static str { pick(self, "목록을 새로고침하지 못했습니다", "Failed to refresh the list") }

    pub fn err_create_item(self, error: &str) -> String {
        match self {
            | Self::Ko => format!("항목 추가에 실패했습니다: {error}"),
            | Self::En => format!("Failed to add the item: {error}"),
        }
    }

    pub fn err_update_item(self, error: &str) -> String {
        match self {
            | Self::Ko => format!("항목 수정에 실패했습니다: {error}"),
            | Self::En => format!("Failed to update the item: {error}"),
        }
    }

    pub fn err_delete_item(self, error: &str) -> String {
        match self {
            | Self::Ko => format!("항목 삭제에 실패했습니다: {error}"),
            | Self::En => format!("Failed to delete the item: {error}"),
        }
    }
}
