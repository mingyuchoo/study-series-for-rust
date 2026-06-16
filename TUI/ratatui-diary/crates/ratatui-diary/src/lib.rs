//! ratatui-diary 애플리케이션 크레이트.
//!
//! 코드는 레이어가 아니라 **기능(feature)** 단위로 구성된다:
//! - [`app`]    : 공유 상태/화면 전환/디스패치 등 오케스트레이션
//! - [`calendar`]: 달력 화면 (상태/업데이트/뷰/키맵)
//! - [`editor`]  : 에디터 화면 (상태/업데이트/뷰/키맵)
//!
//! 범용 텍스트 편집·마크다운 렌더링·파일 저장은 워크스페이스의 별도
//! 라이브러리 크레이트로 분리되어 있다.

pub mod app;
pub mod calendar;
pub mod editor;
pub mod message;

// 추출된 라이브러리 크레이트를 기존 모듈 경로로 재노출
pub use app::Model;
pub use diary_storage as storage;
pub use markdown_render as markdown;
pub use message::Msg;

/// 기존 공개 경로(`ratatui_diary::model::*`) 호환을 위한 facade.
/// 실제 타입은 각 기능 모듈에 정의되어 있다.
pub mod model {
    pub use crate::{app::{DiaryIndex,
                          Model,
                          Screen},
                    calendar::state::CalendarState,
                    editor::state::{EditorState,
                                    EditorSubMode,
                                    Selection}};
}

/// 기존 공개 경로(`ratatui_diary::update::*`) 호환을 위한 facade.
pub mod update {
    pub use crate::app::update::{Command,
                                 update};
}

/// 기존 공개 경로(`ratatui_diary::view::*`) 호환을 위한 facade.
pub mod view {
    pub use crate::{app::view::view,
                    calendar::view::build_calendar_keybindings,
                    editor::view::build_editor_keybindings};
}
