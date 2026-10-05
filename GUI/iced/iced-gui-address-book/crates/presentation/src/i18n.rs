//! 한국어/영어 UI 문자열 사전(i18n).
//!
//! 뷰는 하드코딩된 문자열 대신 [`Language::strings`] 가 돌려주는 [`Strings`] 만
//! 참조한다. 새 언어를 추가하려면 [`Language`] 변형과 정적 [`Strings`] 하나를
//! 추가하면 된다.

use application::error::AppError;
use domain::error::{RepositoryError,
                    ValidationError};

/// 화면 표시 언어.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    Korean,
    #[default]
    English,
}

impl Language {
    /// 토글 버튼에 나열할 순서.
    pub const ALL: [Language; 2] = [Language::Korean, Language::English];

    /// 토글 버튼 라벨. 현재 언어와 무관하게 각 언어의 자국어 이름으로 표시한다.
    pub const fn label(self) -> &'static str {
        match self {
            | Language::Korean => "한국어",
            | Language::English => "English",
        }
    }

    /// 해당 언어의 UI 문자열 사전.
    pub const fn strings(self) -> &'static Strings {
        match self {
            | Language::Korean => &KO,
            | Language::English => &EN,
        }
    }

    /// Translate typed errors at render time so switching languages updates an
    /// existing banner.
    pub fn error_message(self, error: &AppError) -> String {
        if self == Self::English {
            return error.to_string();
        }
        match error {
            | AppError::Validation(ValidationError::EmptyName) => "이름을 입력하세요.".into(),
            | AppError::Validation(ValidationError::EmptyPhone) => "전화번호를 입력하세요.".into(),
            | AppError::Validation(ValidationError::InvalidEmail(email)) => format!("올바른 이메일 주소를 입력하세요: {email}"),
            | AppError::Repository(RepositoryError::MissingId) => "저장된 연락처를 선택하세요.".into(),
            | AppError::Repository(RepositoryError::Backend(detail)) => format!("저장소 오류: {detail}"),
        }
    }

    /// "N contacts" / "연락처 N개" 형태의 개수 문자열.
    pub fn contacts_count(self, count: usize) -> String {
        match self {
            | Language::Korean => format!("연락처 {count}개"),
            | Language::English => format!("{count} {}", if count == 1 { "contact" } else { "contacts" }),
        }
    }
}

/// 화면에 표시되는 모든 정적 문자열.
#[derive(Debug)]
pub struct Strings {
    pub window_title: &'static str,
    pub app_title: &'static str,
    pub nav_badge: &'static str,
    pub hero_subtitle: &'static str,
    pub directory: &'static str,

    pub badge_new: &'static str,
    pub badge_editing: &'static str,
    pub form_title_new: &'static str,
    pub form_title_edit: &'static str,
    pub form_desc_new: &'static str,
    pub form_desc_edit: &'static str,

    pub label_name: &'static str,
    pub placeholder_name: &'static str,
    pub label_phone: &'static str,
    pub placeholder_phone: &'static str,
    pub label_email: &'static str,
    pub placeholder_email: &'static str,
    pub label_address: &'static str,
    pub placeholder_address: &'static str,

    pub button_add: &'static str,
    pub button_save: &'static str,
    pub button_cancel: &'static str,
    pub button_edit: &'static str,
    pub button_delete: &'static str,

    pub empty_title: &'static str,
    pub empty_desc: &'static str,

    pub meta_phone: &'static str,
    pub meta_email: &'static str,
    pub meta_address: &'static str,
    pub error_prefix: &'static str,

    pub footer_title: &'static str,
    pub footer_desc: &'static str,

    pub language_label: &'static str,
    pub theme_label: &'static str,
    pub theme_system: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
}

static EN: Strings = Strings {
    window_title: "Address Book — Cal.com SaaS",
    app_title: "Address Book",
    nav_badge: "Directory",
    hero_subtitle: "Manage your contacts with a clean, modern directory interface.",
    directory: "Directory",

    badge_new: "NEW CONTACT",
    badge_editing: "EDITING CONTACT",
    form_title_new: "Create New Contact",
    form_title_edit: "Edit Contact Details",
    form_desc_new: "Fill in the contact fields to register an entry in your directory.",
    form_desc_edit: "Update the contact information below and save your changes.",

    label_name: "Full Name",
    placeholder_name: "e.g. Jane Doe",
    label_phone: "Phone Number",
    placeholder_phone: "e.g. +1 (555) 0123",
    label_email: "Email Address",
    placeholder_email: "e.g. jane@example.com",
    label_address: "Physical Address",
    placeholder_address: "e.g. 123 Market St, Suite 400",

    button_add: "Add Contact",
    button_save: "Save Changes",
    button_cancel: "Cancel",
    button_edit: "Edit",
    button_delete: "Delete",

    empty_title: "No contacts saved yet",
    empty_desc: "Use the form above to register your first address entry.",

    meta_phone: "Phone",
    meta_email: "Email",
    meta_address: "Address",
    error_prefix: "Error",

    footer_title: "Address Book Directory",
    footer_desc: "Engineered with Rust & Iced • Cal.com Design System",

    language_label: "Language",
    theme_label: "Theme",
    theme_system: "System",
    theme_light: "Light",
    theme_dark: "Dark",
};

static KO: Strings = Strings {
    window_title: "주소록 — Cal.com SaaS",
    app_title: "주소록",
    nav_badge: "디렉터리",
    hero_subtitle: "깔끔하고 모던한 디렉터리 인터페이스로 연락처를 관리하세요.",
    directory: "디렉터리",

    badge_new: "새 연락처",
    badge_editing: "연락처 편집 중",
    form_title_new: "새 연락처 만들기",
    form_title_edit: "연락처 정보 수정",
    form_desc_new: "연락처 정보를 입력해 디렉터리에 등록하세요.",
    form_desc_edit: "아래 연락처 정보를 수정한 뒤 변경 사항을 저장하세요.",

    label_name: "이름",
    placeholder_name: "예: 홍길동",
    label_phone: "전화번호",
    placeholder_phone: "예: 010-1234-5678",
    label_email: "이메일 주소",
    placeholder_email: "예: gildong@example.com",
    label_address: "주소",
    placeholder_address: "예: 서울특별시 중구 세종대로 110",

    button_add: "연락처 추가",
    button_save: "변경 사항 저장",
    button_cancel: "취소",
    button_edit: "편집",
    button_delete: "삭제",

    empty_title: "저장된 연락처가 없습니다",
    empty_desc: "위 입력 폼을 사용해 첫 번째 주소를 등록하세요.",

    meta_phone: "전화",
    meta_email: "이메일",
    meta_address: "주소",
    error_prefix: "오류",

    footer_title: "주소록 디렉터리",
    footer_desc: "Rust & Iced 로 제작 • Cal.com 디자인 시스템",

    language_label: "언어",
    theme_label: "테마",
    theme_system: "시스템",
    theme_light: "라이트",
    theme_dark: "다크",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_language_has_its_own_strings() {
        assert_eq!(Language::English.strings().app_title, "Address Book");
        assert_eq!(Language::Korean.strings().app_title, "주소록");
        assert_eq!(Language::default(), Language::English);
    }

    #[test]
    fn errors_are_translated_when_rendered() {
        let error = AppError::Validation(ValidationError::EmptyName);
        assert_eq!(Language::English.error_message(&error), "name must not be empty");
        assert_eq!(Language::Korean.error_message(&error), "이름을 입력하세요.");
        let error = AppError::Repository(RepositoryError::Backend("SQLite detail".into()));
        assert_eq!(Language::Korean.error_message(&error), "저장소 오류: SQLite detail");
    }

    #[test]
    fn contacts_count_is_localized() {
        assert_eq!(Language::English.contacts_count(3), "3 contacts");
        assert_eq!(Language::English.contacts_count(1), "1 contact");
        assert_eq!(Language::Korean.contacts_count(3), "연락처 3개");
    }
}
