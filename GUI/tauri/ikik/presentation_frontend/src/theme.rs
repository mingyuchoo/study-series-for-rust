//! 라이트/다크 테마 상태. `<html data-theme="...">` 속성으로 CSS 변수
//! 팔레트를 전환하고, 사용자가 토글한 선택은 localStorage에 보존한다.
//! 저장된 선택이 없으면 OS 설정(prefers-color-scheme)을 따른다.

const STORAGE_KEY: &str = "theme";

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

    fn as_str(self) -> &'static str {
        match self {
            | Self::Light => "light",
            | Self::Dark => "dark",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            | "light" => Some(Self::Light),
            | "dark" => Some(Self::Dark),
            | _ => None,
        }
    }
}

/// 시작 테마: 저장된 사용자 선택 > OS 설정 > 라이트.
pub fn initial_theme() -> Theme {
    let Some(window) = web_sys::window() else {
        return Theme::Light;
    };

    let stored = window
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten())
        .and_then(|value| Theme::from_str(&value));
    if let Some(theme) = stored {
        return theme;
    }

    let prefers_dark = window
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches());
    if prefers_dark { Theme::Dark } else { Theme::Light }
}

/// 문서 루트에 테마를 반영하고 선택을 저장한다.
pub fn apply_theme(theme: Theme) {
    let Some(window) = web_sys::window() else {
        return;
    };

    if let Some(root) = window.document().and_then(|document| document.document_element()) {
        let _ = root.set_attribute("data-theme", theme.as_str());
    }
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item(STORAGE_KEY, theme.as_str());
    }
}
