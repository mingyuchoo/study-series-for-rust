use chrono::NaiveDate;
use std::ops::{Deref,
               DerefMut};
// 텍스트 에디터 엔진의 타입을 그대로 재노출 (기존 경로 호환)
pub use text_editor::Selection;
use text_editor::TextEditor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorSubMode {
    CtrlX,
    Search,
}

/// 다이어리 화면의 에디터 상태.
///
/// 범용 텍스트 편집 기능은 [`TextEditor`] 엔진에 위임하고
/// (`Deref`/`DerefMut`로 투명하게 노출), 다이어리 고유의 상태인
/// 편집 대상 날짜와 키 서브모드만 직접 보유한다.
pub struct EditorState {
    pub editor: TextEditor,
    pub date: NaiveDate,
    pub submode: Option<EditorSubMode>,
}

impl EditorState {
    pub fn new(date: NaiveDate) -> Self {
        Self {
            editor: TextEditor::new(),
            date,
            submode: None,
        }
    }
}

impl Deref for EditorState {
    type Target = TextEditor;

    fn deref(&self) -> &Self::Target { &self.editor }
}

impl DerefMut for EditorState {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.editor }
}
