//! 한국어/영어 UI 문자열. 언어는 `Signal<Lang>` 컨텍스트로 내려가고,
//! 토글한 선택은 localStorage와 `<html lang>` 속성에 보존된다.
//! 모든 문자열이 `Lang` 메서드라서 번역 누락이 컴파일 에러로 잡힌다.
//! 백엔드가 만드는 에러 메시지는 1차 범위 밖이라 한국어로 내려온다.

use dioxus::prelude::*;

const STORAGE_KEY: &str = "lang";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ko,
    En,
}

/// `App`이 제공한 언어 시그널. 읽는 컴포넌트는 언어가 바뀌면 다시 그린다.
pub fn use_lang() -> Signal<Lang> { use_context::<Signal<Lang>>() }

/// 시작 언어: 저장된 사용자 선택 > 한국어.
pub fn initial_lang() -> Lang {
    let stored = web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten());
    match stored.as_deref() {
        | Some("en") => Lang::En,
        | _ => Lang::Ko,
    }
}

/// `<html lang>`을 갱신하고 선택을 저장한다.
pub fn apply_lang(lang: Lang) {
    let Some(window) = web_sys::window() else {
        return;
    };

    if let Some(root) = window.document().and_then(|document| document.document_element()) {
        let _ = root.set_attribute("lang", lang.html_code());
    }
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item(STORAGE_KEY, lang.html_code());
    }
}

fn pick(lang: Lang, ko: &'static str, en: &'static str) -> &'static str {
    match lang {
        | Lang::Ko => ko,
        | Lang::En => en,
    }
}

impl Lang {
    pub fn toggled(self) -> Self {
        match self {
            | Self::Ko => Self::En,
            | Self::En => Self::Ko,
        }
    }

    fn html_code(self) -> &'static str { pick(self, "ko", "en") }

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

    // ── 대시보드 ─────────────────────────────────────────────
    pub fn dash_filter_notice(self) -> &'static str { pick(self, "검색 결과 기준 집계입니다.", "Aggregated from search results.") }

    pub fn all_kpis(self) -> &'static str { pick(self, "전체 핵심 성과 지표", "All Key Performance Indicators") }

    pub fn kpi_levels_heading(self) -> &'static str { pick(self, "핵심 성과 지표 현재 수준", "Key Performance Indicator current levels") }

    pub fn dash_average(self, average: i64) -> String {
        match self {
            | Self::Ko => format!("평균 달성률 {average}%"),
            | Self::En => format!("Average progress {average}%"),
        }
    }

    pub fn dash_no_measurable(self) -> &'static str {
        pick(
            self,
            "달성률을 계산할 핵심 성과 지표가 없습니다.",
            "No Key Performance Indicators with a computable progress.",
        )
    }

    pub fn dash_no_target_suffix(self, count: usize) -> String {
        match self {
            | Self::Ko => format!(" · 목표 미설정 {count}개"),
            | Self::En => format!(" · {count} without a target"),
        }
    }

    pub fn no_kpis(self) -> &'static str { pick(self, "핵심 성과 지표가 없습니다.", "No Key Performance Indicators.") }

    pub fn no_target(self) -> &'static str { pick(self, "목표 미설정", "No target") }

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

    pub fn value_placeholder(self) -> &'static str { pick(self, "측정값", "Value") }

    // ── 측정값 스테퍼 ────────────────────────────────────────
    pub fn step_increase(self) -> &'static str { pick(self, "측정값 늘리기", "Increase value") }

    pub fn step_decrease(self) -> &'static str { pick(self, "측정값 줄이기", "Decrease value") }

    pub fn step_size_aria(self) -> &'static str { pick(self, "한 클릭의 변화량 선택", "Select the step size") }

    pub fn value_must_be_number(self) -> &'static str { pick(self, "측정값은 숫자로 입력하세요.", "The value must be a number.") }

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

    // ── 단계 탭 표 ───────────────────────────────────────────
    pub fn empty_lane(self) -> &'static str { pick(self, "비어 있음", "Empty") }

    // ── 빠른 추가 ────────────────────────────────────────────
    pub fn quick_add_placeholder(self, kind_label: &str) -> String {
        match self {
            | Self::Ko => format!("{kind_label} 제목을 입력하고 Enter"),
            | Self::En => format!("Type a {kind_label} title and press Enter"),
        }
    }

    pub fn more_details(self) -> &'static str { pick(self, "자세히 입력", "More details") }

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

    // ── 전체 구조 트리 ───────────────────────────────────────
    pub fn tree_empty_hint(self) -> &'static str {
        pick(
            self,
            "아직 IKIK 항목이 없습니다. 아래에서 정체성부터 추가해 보세요.",
            "No IKIK items yet. Start by adding an Identity below.",
        )
    }

    pub fn expand_all(self) -> &'static str { pick(self, "모두 펼치기", "Expand all") }

    pub fn collapse_all(self) -> &'static str { pick(self, "모두 접기", "Collapse all") }

    pub fn tree_flow_hint(self) -> &'static str {
        pick(
            self,
            "정체성 → 핵심 결과 영역 → 소득 창출 업무 → 핵심 성과 지표 · 행을 끌어 새 상위 항목 위에 놓으면 이동합니다",
            "Identity → Key Result Area → Income Generating Task → Key Performance Indicator · drag a row onto a new parent to move it",
        )
    }

    pub fn add_identity(self) -> &'static str { pick(self, "+ 정체성 추가", "+ Add Identity") }

    pub fn collapse(self) -> &'static str { pick(self, "접기", "Collapse") }

    pub fn expand(self) -> &'static str { pick(self, "펼치기", "Expand") }

    pub fn nested_count(self, count: usize) -> String {
        match self {
            | Self::Ko => format!("하위 {count}"),
            | Self::En => format!("{count} nested"),
        }
    }
}
