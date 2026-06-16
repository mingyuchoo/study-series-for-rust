//! 애플리케이션 오케스트레이션 계층.
//!
//! 공유 상태([`Model`])와 화면 전환([`Screen`])을 보유하고, 메시지/키/
//! 렌더링을 각 기능 모듈(`calendar`, `editor`)로 위임한다. 파일 I/O 같은
//! 부수효과는 [`Command`]로 분리되어 [`runtime::execute_command`]가 실행한다.

use crate::{calendar::state::CalendarState,
            editor::state::EditorState};
use chrono::{Datelike,
             NaiveDate};
use diary_storage::Storage;
use std::collections::HashSet;

pub mod keymap;
pub mod runtime;
pub mod update;
pub mod view;

pub use keymap::handle_key;
pub use runtime::execute_command;
pub use update::{Command,
                 update};
pub use view::view;

pub struct Model {
    pub screen: Screen,
    pub calendar_state: CalendarState,
    pub editor_state: EditorState,
    pub diary_entries: DiaryIndex,
    pub error_message: Option<String>,
    pub show_error_popup: bool,
    pub storage: Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Calendar,
    Editor,
}

pub struct DiaryIndex {
    pub entries: HashSet<NaiveDate>,
}

impl Model {
    pub fn new(entries: HashSet<NaiveDate>, storage: Storage) -> Self {
        let today = chrono::Local::now().date_naive();

        Self {
            screen: Screen::Calendar,
            calendar_state: CalendarState::new(today.year(), today.month()),
            editor_state: EditorState::new(today),
            diary_entries: DiaryIndex {
                entries,
            },
            error_message: None,
            show_error_popup: false,
            storage,
        }
    }
}
