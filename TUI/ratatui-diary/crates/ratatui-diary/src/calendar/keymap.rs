//! 달력 화면의 키 입력 → 메시지 매핑.

use crate::message::Msg;
use crossterm::event::{KeyCode,
                       KeyEvent,
                       KeyModifiers};

pub fn handle_key(key: KeyEvent) -> Option<Msg> {
    let mods = key.modifiers;

    match key.code {
        // Ctrl+Q: 종료
        | KeyCode::Char('q') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),

        // Ctrl+B/F/P/N: 이동
        | KeyCode::Char('b') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::CalendarMoveLeft),
        | KeyCode::Char('f') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::CalendarMoveRight),
        | KeyCode::Char('p') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::CalendarMoveUp),
        | KeyCode::Char('n') if mods.contains(KeyModifiers::CONTROL) => Some(Msg::CalendarMoveDown),

        // Enter: 편집
        | KeyCode::Enter => Some(Msg::CalendarSelectDate),

        // Alt+N / Alt+P: 다음/이전 달
        | KeyCode::Char('n') if mods.contains(KeyModifiers::ALT) => Some(Msg::CalendarNextMonth),
        | KeyCode::Char('p') if mods.contains(KeyModifiers::ALT) => Some(Msg::CalendarPrevMonth),

        // Alt+] / Alt+[: 다음/이전 년
        | KeyCode::Char(']') if mods.contains(KeyModifiers::ALT) => Some(Msg::CalendarNextYear),
        | KeyCode::Char('[') if mods.contains(KeyModifiers::ALT) => Some(Msg::CalendarPrevYear),

        | _ => None,
    }
}
