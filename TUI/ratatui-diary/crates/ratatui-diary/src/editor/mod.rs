//! 에디터 화면 기능: 상태 / 메시지 처리 / 렌더링 / 키 매핑.
//!
//! 범용 텍스트 편집 로직은 `text-editor` 크레이트의 엔진에 위임하며,
//! 여기서는 다이어리 고유의 편집 화면 동작만 다룬다.

pub mod keymap;
pub mod state;
pub mod update;
pub mod view;

pub use state::{EditorState,
                EditorSubMode,
                Selection};
