//! 라이트/다크 테마 상태. `<html data-theme="...">` 속성으로 CSS 변수
//! 팔레트를 전환하고, 사용자가 토글한 선택은 localStorage에 보존한다.
//! 저장된 선택이 없으면 OS 설정(prefers-color-scheme)을 따른다.

use crate::preferences::PersistedToggle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub fn toggled(self) -> Self {
        match self {
            | Self::Light => Self::Dark,
            | Self::Dark => Self::Light,
        }
    }
}

impl PersistedToggle for Theme {
    const HTML_ATTR: Option<&'static str> = Some("data-theme");
    const STORAGE_KEY: &'static str = "theme";

    fn as_storage_value(self) -> &'static str {
        match self {
            | Self::Light => "light",
            | Self::Dark => "dark",
        }
    }

    fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            | "light" => Some(Self::Light),
            | "dark" => Some(Self::Dark),
            | _ => None,
        }
    }

    /// 저장된 선택이 없을 때의 기본값: OS 설정(prefers-color-scheme) > 라이트.
    fn fallback() -> Self {
        let prefers_dark = web_sys::window()
            .and_then(|window| window.match_media("(prefers-color-scheme: dark)").ok().flatten())
            .is_some_and(|query| query.matches());
        if prefers_dark { Self::Dark } else { Self::Light }
    }
}

/// 시작 테마: 저장된 사용자 선택 > OS 설정 > 라이트.
pub fn initial_theme() -> Theme { Theme::initial() }

/// 문서 루트에 테마를 반영하고 선택을 저장한다.
pub fn apply_theme(theme: Theme) { theme.apply(); }
