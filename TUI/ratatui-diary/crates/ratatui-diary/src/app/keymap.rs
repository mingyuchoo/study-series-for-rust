//! 최상위 키 입력 디스패처.
//!
//! 전역 키(에러 팝업 닫기)를 먼저 처리하고, 그 외에는 현재 화면의
//! 기능 모듈 키맵으로 위임한다.

use crate::{app::{Model,
                  Screen},
            calendar,
            editor,
            message::Msg};
use crossterm::event::{KeyCode,
                       KeyEvent,
                       KeyEventKind};

pub fn handle_key(key: KeyEvent, model: &Model) -> Option<Msg> {
    // Windows reports both key press and key release events. Handling the
    // release as input would insert every character twice.
    if key.kind == KeyEventKind::Release {
        return None;
    }

    // 에러 팝업이 표시 중이면 Esc로 닫기
    if model.show_error_popup && key.code == KeyCode::Esc {
        return Some(Msg::DismissError);
    }

    match model.screen {
        | Screen::Calendar => calendar::keymap::handle_key(key),
        | Screen::Editor => editor::keymap::handle_key(key, &model.editor_state),
    }
}
