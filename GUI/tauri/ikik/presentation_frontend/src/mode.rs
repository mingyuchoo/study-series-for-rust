//! 사용/관리 모드 상태. 사용 모드(기본)에서는 수정·삭제·추가·이동
//! 같은 구조 변경 진입점이 화면에서 사라져 실수로 데이터를 잃는 것을
//! 막고, 조회·검색·실적 기록만 남는다. 관리 모드는 헤더의 자물쇠
//! 토글로 열고, 선택은 테마·언어처럼 localStorage에 보존한다.

use dioxus::prelude::*;

const STORAGE_KEY: &str = "mode";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 조회·검색·실적 기록만. 구조 변경 진입점은 숨긴다.
    Use,
    /// 수정·삭제·추가·드래그 이동까지 전부.
    Manage,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            | Self::Use => Self::Manage,
            | Self::Manage => Self::Use,
        }
    }

    pub fn is_manage(self) -> bool { self == Self::Manage }

    fn as_str(self) -> &'static str {
        match self {
            | Self::Use => "use",
            | Self::Manage => "manage",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            | "use" => Some(Self::Use),
            | "manage" => Some(Self::Manage),
            | _ => None,
        }
    }
}

/// `App`이 제공한 모드 시그널. 읽는 컴포넌트는 모드가 바뀌면 다시 그린다.
pub fn use_mode() -> Signal<Mode> { use_context::<Signal<Mode>>() }

/// 시작 모드: 저장된 사용자 선택 > 사용 모드(안전 기본값).
pub fn initial_mode() -> Mode {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten())
        .and_then(|value| Mode::from_str(&value))
        .unwrap_or(Mode::Use)
}

/// 선택을 저장한다.
pub fn apply_mode(mode: Mode) {
    if let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten()) {
        let _ = storage.set_item(STORAGE_KEY, mode.as_str());
    }
}
