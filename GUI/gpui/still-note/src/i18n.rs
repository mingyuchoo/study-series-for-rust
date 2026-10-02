use crate::settings::Language;
use std::fmt;
impl Language {
    /// Catalog lookups apply only to application-owned text, never user content.
    pub fn text(self, key: &str) -> &str {
        if self == Self::English {
            if let Some((_, english)) = TEXT.iter().find(|(k, _)| *k == key) {
                return english;
            }
        }
        key
    }
}
#[derive(Debug)]
pub struct Message {
    pub key: &'static str,
    pub detail: Option<String>,
}
impl Message {
    pub fn new(key: &'static str) -> Self {
        Self { key, detail: None }
    }
    pub fn detail(key: &'static str, detail: impl ToString) -> Self {
        Self {
            key,
            detail: Some(detail.to_string()),
        }
    }
    pub fn localized(&self, language: Language) -> String {
        match &self.detail {
            | Some(detail) => format!("{}: {detail}", language.text(self.key)),
            | None => language.text(self.key).to_owned(),
        }
    }
}
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.localized(Language::Korean))
    }
}
impl std::error::Error for Message {}
pub fn error_text(error: &anyhow::Error, language: Language) -> String {
    error
        .chain()
        .map(|cause| {
            cause
                .downcast_ref::<Message>()
                .map(|message| message.localized(language))
                .unwrap_or_else(|| cause.to_string())
        })
        .collect::<Vec<_>>()
        .join(": ")
}
const TEXT: &[(&str, &str)] = &[
    ("Stillnote · 나의 불렛저널", "Stillnote · My bullet journal"),
    ("머릿속의 생각을 한 줄로 기록해 보세요", "Capture a thought in one line"),
    ("전체 기록 검색…", "Search all entries…"),
    ("새 컬렉션 이름", "New collection name"),
    ("대상 날짜 YYYY-MM-DD", "Target date YYYY-MM-DD"),
    ("기록은 이 컴퓨터에만 저장됩니다", "Your entries stay on this computer"),
    (
        "원본 파일 보호를 위해 읽기 전용입니다. README의 복구 안내를 확인해 주세요",
        "Read-only to protect the original file. See recovery instructions in README",
    ),
    ("로컬 저장 완료", "Saved locally"),
    ("저장하지 못했습니다", "Unable to save"),
    ("올바른 날짜를 입력해 주세요 (YYYY-MM-DD)", "Enter a valid date (YYYY-MM-DD)"),
    ("이월 대상 날짜를 확인해 주세요", "Check the migration target date"),
    ("인덱스", "Index"),
    ("일간 로그", "Daily log"),
    ("월간 로그", "Monthly log"),
    ("미래 로그", "Future log"),
    ("컬렉션", "Collections"),
    ("일간", "Daily"),
    ("월간", "Monthly"),
    ("미래", "Future"),
    ("재개", "Reopen"),
    ("완료", "Complete"),
    ("수정", "Edit"),
    ("취소", "Cancel"),
    ("이월 →", "Migrate →"),
    ("로그 열기", "Open log"),
    ("이월됨", "Migrated"),
    ("원본", "Source"),
    ("✕ 메뉴 닫기", "✕ Close menu"),
    ("☰ 메뉴", "☰ Menu"),
    ("+ 만들기", "+ Create"),
    ("+ 컬렉션 만들기", "+ Create collection"),
    ("나의 기록", "My entries"),
    ("검색 결과.", "Search results."),
    ("기록이 쌓이면 나만의 지도가 됩니다", "Your entries become your own map"),
    ("모든 기록", "All entries"),
    ("미완료", "Open"),
    ("읽기 전용 · 원본 파일을 확인해 주세요", "Read-only · Check the original file"),
    ("오류 · 입력을 유지했습니다", "Error · Your input is preserved"),
    ("이동", "Go"),
    ("오늘", "Today"),
    ("기록 수정 중", "Editing entry"),
    ("수정 취소", "Cancel edit"),
    ("• 할 일", "• Task"),
    ("○ 이벤트", "○ Event"),
    ("– 메모", "– Note"),
    ("수정 저장", "Save edit"),
    ("기록 +", "Add +"),
    ("이 할 일은 언제 다시 할까요?", "When will you return to this task?"),
    ("이월 저장", "Save migration"),
    ("닫기", "Close"),
    (
        "아직 인덱스가 비어 있어요. 오늘의 첫 기록을 남겨 보세요.",
        "Your index is empty. Add your first entry today.",
    ),
    (
        "이번 달의 달력 · 날짜를 누르면 일간 로그가 열립니다",
        "This month · Select a date to open its daily log",
    ),
    ("월", "Mon"),
    ("화", "Tue"),
    ("수", "Wed"),
    ("목", "Thu"),
    ("금", "Fri"),
    ("토", "Sat"),
    ("일", "Sun"),
    ("이번 달의 할 일과 기록", "This month’s tasks and entries"),
    ("찾은 기록이 없습니다.", "No entries found."),
    ("작은 기록으로 시작하세요.", "Start with a small entry."),
    ("검색어 또는 상태 필터를 바꿔 보세요.", "Try a different search or status filter."),
    (
        "해야 할 일, 있었던 일, 기억하고 싶은 생각.\n한 줄이면 충분해요. 입력 후 Enter로 저장합니다.",
        "Tasks, events, and thoughts worth keeping.\nOne line is enough. Press Enter to save.",
    ),
    ("Enter 기록 · Ctrl+A 선택 · Ctrl+V 붙여넣기", "Enter save · Ctrl+A select · Ctrl+V paste"),
    ("열린 할 일", "Open tasks"),
    ("천천히 돌아보기.", "Take a moment to reflect."),
    (
        "오늘의 기록이 내일의 방향이 됩니다. 필요한 일만 다음으로 가져가세요.",
        "Today’s entries guide tomorrow. Carry forward only what matters.",
    ),
    ("빠른 기록 범례", "Quick entry guide"),
    ("•   해야 할 일", "•   Task"),
    ("×   완료한 일", "×   Completed"),
    ("○   이벤트", "○   Event"),
    ("–   생각과 메모", "–   Thought or note"),
    (">   다른 로그로 이월", ">   Migrated to another log"),
    ("<   미래 로그에 예약", "<   Scheduled in the future log"),
    ("★   중요한 기록", "★   Important entry"),
    ("⊘   취소한 기록", "⊘   Cancelled entry"),
    (
        "Ryder Carroll의 불렛저널 방법에서 영감을 받았습니다.",
        "Inspired by Ryder Carroll’s Bullet Journal method.",
    ),
    ("사용자 데이터 경로를 찾을 수 없습니다", "User data directory unavailable"),
    (
        "저널 파일을 읽을 수 없습니다. 원본을 보존했습니다",
        "Cannot read the journal. The original is preserved",
    ),
    (
        "저널 데이터 검증 실패. 원본을 보존했습니다",
        "Journal validation failed. The original is preserved",
    ),
    (
        "저널 파일 접근 실패. 원본을 보존했습니다",
        "Cannot access the journal. The original is preserved",
    ),
    (
        "손상된 파일 보호: 정상 로드 전에는 저장할 수 없습니다",
        "File protected: cannot save before a successful load",
    ),
    ("기존 파일 확인 실패", "Cannot check the existing file"),
    (
        "다른 프로그램이 파일을 변경했습니다. 앱을 재시작해 주세요",
        "Another application changed the journal. Restart the app",
    ),
    ("데이터 폴더 생성 실패", "Cannot create the data directory"),
    ("백업 저장 실패", "Cannot save the backup"),
    ("저널 파일 교체 실패", "Cannot replace the journal file"),
    ("날짜는 0001~9999년 범위여야 합니다", "Date must be in years 0001–9999"),
    ("월 범위를 벗어났습니다", "Month is outside the supported range"),
    ("지원하지 않는 저장 버전", "Unsupported journal version"),
    ("중복 컬렉션 이름", "Duplicate collection name"),
    ("컬렉션 ID/이름이 올바르지 않습니다", "Invalid collection ID or name"),
    ("중복 또는 빈 항목 ID", "Duplicate or empty entry ID"),
    ("항목 텍스트/날짜가 올바르지 않습니다", "Invalid entry text or date"),
    ("메모/이벤트에 작업 상태가 적용되었습니다", "Task status applied to a note or event"),
    ("이월 상태와 연결이 다릅니다", "Migration status does not match its link"),
    ("잘못된 이월 대상 연결", "Invalid migration target link"),
    ("예약 상태와 대상 로그가 다릅니다", "Scheduled status does not match the target log"),
    ("잘못된 이월 원본 연결", "Invalid migration source link"),
    ("이월 연결에 순환이 있습니다", "Migration links contain a cycle"),
    ("컬렉션을 찾을 수 없습니다", "Collection not found"),
    ("항목을 찾을 수 없습니다", "Entry not found"),
    ("컬렉션 이름을 입력해 주세요", "Enter a collection name"),
    ("이미 있는 컬렉션 이름입니다", "Collection name already exists"),
    ("기록할 내용을 입력해 주세요", "Enter some content"),
    ("잘못된 날짜입니다", "Invalid date"),
    ("이월한 원본은 변경할 수 없습니다", "Cannot change a migrated source entry"),
    ("이월 기능을 사용해 주세요", "Use the migration command"),
    ("할 일만 완료할 수 있습니다", "Only tasks can be completed"),
    ("빈 기록은 저장할 수 없습니다", "Cannot save an empty entry"),
    ("이월 원본은 수정할 수 없습니다", "Cannot edit a migrated source entry"),
    ("미완료 할 일만 이월할 수 있습니다", "Only open tasks can be migrated"),
    ("다른 날짜 또는 로그를 선택해 주세요", "Choose a different date or log"),
    (
        "이월 대상은 일간/월간/미래 로그입니다",
        "Migration target must be a daily, monthly, or future log",
    ),
    (
        "설정 파일을 읽을 수 없습니다. 원본을 보존했습니다",
        "Cannot read settings. The original is preserved",
    ),
    ("설정 파일 접근 실패. 원본을 보존했습니다", "Cannot access settings. The original is preserved"),
    (
        "설정 원본 보호: 파일을 확인한 뒤 앱을 재시작해 주세요",
        "Settings protected: check the file and restart the app",
    ),
    (
        "다른 프로그램이 설정을 변경했습니다. 앱을 재시작해 주세요",
        "Another application changed settings. Restart the app",
    ),
    (
        "설정을 저장하지 못했습니다. 선택은 현재 세션에 적용됩니다",
        "Cannot save settings. Your selection applies to this session",
    ),
    ("설정", "Settings"),
    ("언어", "Language"),
    ("테마", "Theme"),
    ("시스템", "System"),
    ("라이트", "Light"),
    ("다크", "Dark"),
];
