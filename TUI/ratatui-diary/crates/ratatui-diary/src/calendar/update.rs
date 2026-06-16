//! 달력 화면의 메시지 처리.
//!
//! 호출 시점에 이미 화면이 [`Screen::Calendar`]임이 보장되므로
//! (최상위 [`crate::app::update`] 디스패처가 화면별로 위임) 별도의
//! 화면 가드 없이 자신의 메시지만 처리한다.

use crate::{app::{Command,
                  Model,
                  Screen},
            message::Msg};

pub fn update(model: &mut Model, msg: Msg) -> Option<Command> {
    match msg {
        | Msg::CalendarMoveLeft => model.calendar_state.move_cursor_left(),
        | Msg::CalendarMoveRight => model.calendar_state.move_cursor_right(),
        | Msg::CalendarMoveUp => model.calendar_state.move_cursor_up(),
        | Msg::CalendarMoveDown => model.calendar_state.move_cursor_down(),
        | Msg::CalendarSelectDate => {
            let date = model.calendar_state.selected_date;
            model.screen = Screen::Editor;
            model.editor_state.date = date;
            return Some(Command::LoadDiary(date));
        },
        | Msg::CalendarNextMonth => model.calendar_state.next_month(),
        | Msg::CalendarPrevMonth => model.calendar_state.prev_month(),
        | Msg::CalendarNextYear => model.calendar_state.next_year(),
        | Msg::CalendarPrevYear => model.calendar_state.prev_year(),
        | _ => {},
    }

    None
}
