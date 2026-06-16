//! [`Command`] 실행 — 파일 I/O 부수효과를 수행하고 결과를 다시
//! 메시지로 피드백하여 [`update`]에 전달한다.

use crate::{app::{Model,
                  update::{self,
                           Command}},
            message::Msg};

pub fn execute_command(cmd: Command, model: &mut Model) -> std::io::Result<()> {
    match cmd {
        | Command::LoadDiary(date) => match model.storage.load(date) {
            | Ok(content) => {
                update::update(model, Msg::LoadDiarySuccess(date, content));
            },
            | Err(e) => {
                update::update(model, Msg::LoadDiaryFailed(e.to_string()));
            },
        },
        | Command::SaveDiary(date, content) => match model.storage.save(date, &content) {
            | Ok(_) => {
                model.diary_entries.entries.insert(date);
                update::update(model, Msg::SaveDiarySuccess);
            },
            | Err(e) => {
                update::update(model, Msg::SaveDiaryFailed(e.to_string()));
            },
        },
        | Command::DeleteDiary(date) => match model.storage.delete(date) {
            | Ok(_) => {
                update::update(model, Msg::DeleteDiarySuccess(date));
            },
            | Err(e) => {
                update::update(model, Msg::SaveDiaryFailed(e.to_string()));
            },
        },
    }

    Ok(())
}
