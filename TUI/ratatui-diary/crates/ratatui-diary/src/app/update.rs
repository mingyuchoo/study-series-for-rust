//! 최상위 메시지 디스패처.
//!
//! 앱 전역 메시지(종료, 에러, 파일 I/O 결과)는 여기서 직접 처리하고,
//! 그 외 화면별 메시지는 현재 [`Screen`]에 따라 기능 모듈로 위임한다.
//! 위임 대상 메시지는 모두 해당 화면에서만 의미가 있으므로(원래 각 처리부가
//! 화면 가드를 갖고 있었다) 화면 기준 라우팅이 동작을 보존한다.

use crate::{app::{Model,
                  Screen},
            calendar,
            editor,
            message::Msg};
use chrono::NaiveDate;

pub enum Command {
    LoadDiary(NaiveDate),
    SaveDiary(NaiveDate, String),
    DeleteDiary(NaiveDate),
}

pub fn update(model: &mut Model, msg: Msg) -> Option<Command> {
    match msg {
        | Msg::Quit => {
            // 메인 루프에서 처리
            None
        },

        | Msg::DismissError => {
            model.show_error_popup = false;
            model.error_message = None;
            None
        },

        // ===== 파일 I/O 결과 =====
        | Msg::LoadDiarySuccess(date, content) => {
            if model.screen == Screen::Editor {
                model.editor_state.date = date;
                model.editor_state.load_content(&content);
            }
            None
        },
        | Msg::LoadDiaryFailed(error) => {
            if !error.contains("No such file") {
                model.error_message = Some(format!("로드 실패: {}", error));
                model.show_error_popup = true;
            }
            None
        },
        | Msg::SaveDiarySuccess => {
            model.editor_state.is_modified = false;
            None
        },
        | Msg::SaveDiaryFailed(error) => {
            model.error_message = Some(format!("저장 실패: {}", error));
            model.show_error_popup = true;
            None
        },
        | Msg::DeleteDiarySuccess(date) => {
            model.diary_entries.entries.remove(&date);
            model.screen = Screen::Calendar;
            None
        },
        | Msg::RefreshIndex(entries) => {
            model.diary_entries.entries = entries;
            None
        },

        // ===== 화면별 메시지 위임 =====
        | other => match model.screen {
            | Screen::Calendar => calendar::update::update(model, other),
            | Screen::Editor => editor::update::update(model, other),
        },
    }
}
