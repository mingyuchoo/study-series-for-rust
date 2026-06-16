//! 에디터 화면의 메시지 처리.
//!
//! 호출 시점에 이미 화면이 [`Screen::Editor`]임이 보장되므로
//! (최상위 [`crate::app::update`] 디스패처가 화면별로 위임) 별도의
//! 화면 가드 없이 자신의 메시지만 처리한다.

use crate::{app::{Command,
                  Model},
            editor::state::{EditorSubMode,
                            Selection},
            message::Msg};
use text_editor::TextEditor;

/// Selection이 활성화되어 있으면 커서 이동시 selection도 업데이트
fn update_selection_on_move(state: &mut TextEditor) {
    if let Some(ref mut sel) = state.selection {
        sel.cursor_line = state.cursor_line;
        sel.cursor_col = state.cursor_col;
    }
}

/// 클립보드 내용을 커서 위치에 붙여넣기
fn paste_clipboard(state: &mut TextEditor) {
    if state.clipboard.is_empty() {
        return;
    }

    if state.clipboard.ends_with('\n') {
        // 줄 단위 붙여넣기
        let lines: Vec<String> = state.clipboard.trim_end_matches('\n').split('\n').map(String::from).collect();

        if state.content.is_empty() {
            state.content.push(String::new());
        }

        let insert_at = (state.cursor_line + 1).min(state.content.len());

        for (i, line) in lines.iter().enumerate() {
            state.content.insert(insert_at + i, line.clone());
        }

        state.cursor_line = insert_at;
        state.cursor_col = 0;
    } else {
        // 문자 단위 붙여넣기 — 커서 위치에 삽입
        if state.cursor_line >= state.content.len() {
            state.content.push(String::new());
        }

        let insert_byte_pos = state.char_idx_to_byte_idx(state.cursor_line, state.cursor_col);
        state.content[state.cursor_line].insert_str(insert_byte_pos, &state.clipboard);
        state.cursor_col += state.clipboard.chars().count();
    }

    state.is_modified = true;
}

pub fn update(model: &mut Model, msg: Msg) -> Option<Command> {
    match msg {
        // ===== 네비게이션 =====
        | Msg::EditorMoveLeft =>
            if model.editor_state.cursor_col > 0 {
                model.editor_state.cursor_col -= 1;
                update_selection_on_move(&mut model.editor_state);
            },
        | Msg::EditorMoveRight => {
            let line_len = if model.editor_state.cursor_line < model.editor_state.content.len() {
                model.editor_state.content[model.editor_state.cursor_line].chars().count()
            } else {
                0
            };
            if model.editor_state.cursor_col < line_len {
                model.editor_state.cursor_col += 1;
                update_selection_on_move(&mut model.editor_state);
            }
        },
        | Msg::EditorMoveUp =>
            if model.editor_state.cursor_line > 0 {
                model.editor_state.cursor_line -= 1;
                let line_len = model.editor_state.content[model.editor_state.cursor_line].chars().count();
                model.editor_state.cursor_col = model.editor_state.cursor_col.min(line_len);
                update_selection_on_move(&mut model.editor_state);
            },
        | Msg::EditorMoveDown =>
            if model.editor_state.cursor_line + 1 < model.editor_state.content.len() {
                model.editor_state.cursor_line += 1;
                let line_len = model.editor_state.content[model.editor_state.cursor_line].chars().count();
                model.editor_state.cursor_col = model.editor_state.cursor_col.min(line_len);
                update_selection_on_move(&mut model.editor_state);
            },
        | Msg::EditorWordNext => {
            model.editor_state.move_word_next();
            update_selection_on_move(&mut model.editor_state);
        },
        | Msg::EditorWordPrev => {
            model.editor_state.move_word_prev();
            update_selection_on_move(&mut model.editor_state);
        },

        // ===== 점프 =====
        | Msg::EditorGotoDocStart => {
            model.editor_state.cursor_line = 0;
            model.editor_state.cursor_col = 0;
            update_selection_on_move(&mut model.editor_state);
        },
        | Msg::EditorGotoDocEnd =>
            if !model.editor_state.content.is_empty() {
                model.editor_state.cursor_line = model.editor_state.content.len() - 1;
                model.editor_state.cursor_col = model.editor_state.content[model.editor_state.cursor_line].chars().count();
                update_selection_on_move(&mut model.editor_state);
            },
        | Msg::EditorGotoLineStart => {
            model.editor_state.cursor_col = 0;
            update_selection_on_move(&mut model.editor_state);
        },
        | Msg::EditorGotoLineEnd => {
            let line_len = if model.editor_state.cursor_line < model.editor_state.content.len() {
                model.editor_state.content[model.editor_state.cursor_line].chars().count()
            } else {
                0
            };
            model.editor_state.cursor_col = line_len;
            update_selection_on_move(&mut model.editor_state);
        },
        | Msg::EditorExitSubMode => model.editor_state.submode = None,

        // ===== 문자 입력 (항상 활성) =====
        | Msg::EditorInsertChar(c) => model.editor_state.insert_char(c),
        | Msg::EditorBackspace => model.editor_state.backspace(),
        | Msg::EditorNewLine => model.editor_state.new_line(),
        | Msg::EditorOpenLine => model.editor_state.open_line(),

        // ===== Selection =====
        | Msg::EditorToggleSelection => {
            let state = &mut model.editor_state;
            if state.selection.is_some() {
                state.selection = None;
            } else {
                state.selection = Some(Selection {
                    anchor_line: state.cursor_line,
                    anchor_col: state.cursor_col,
                    cursor_line: state.cursor_line,
                    cursor_col: state.cursor_col,
                });
            }
        },
        | Msg::EditorSelectLine => {
            let state = &mut model.editor_state;
            let line_len = if state.cursor_line < state.content.len() {
                state.content[state.cursor_line].chars().count()
            } else {
                0
            };
            state.selection = Some(Selection {
                anchor_line: state.cursor_line,
                anchor_col: 0,
                cursor_line: state.cursor_line,
                cursor_col: line_len,
            });
        },

        // ===== 편집 기능 =====
        | Msg::EditorDelete =>
        // Selection이 있을 때만 삭제
            if model.editor_state.selection.is_some() {
                if let Some(text) = model.editor_state.get_selected_text() {
                    model.editor_state.clipboard = text;
                }
                model.editor_state.delete_selection();
                model.editor_state.save_snapshot();
            },
        | Msg::EditorDeleteForward => {
            model.editor_state.delete_forward();
            model.editor_state.save_snapshot();
        },
        | Msg::EditorKillLine => {
            let killed = model.editor_state.kill_line();
            if !killed.is_empty() {
                model.editor_state.clipboard = killed;
            }
            model.editor_state.save_snapshot();
        },
        | Msg::EditorYank =>
            if let Some(text) = model.editor_state.get_selected_text() {
                model.editor_state.clipboard = text;
                model.editor_state.selection = None;
            },
        | Msg::EditorPaste => {
            paste_clipboard(&mut model.editor_state);
            model.editor_state.save_snapshot();
        },

        // ===== Undo/Redo =====
        | Msg::EditorUndo => model.editor_state.undo(),
        | Msg::EditorRedo => model.editor_state.redo(),

        // ===== 검색 =====
        | Msg::EditorEnterSearchMode => {
            model.editor_state.submode = Some(EditorSubMode::Search);
            model.editor_state.search_pattern.clear();
        },
        | Msg::EditorSearchChar(c) =>
            if model.editor_state.submode == Some(EditorSubMode::Search) {
                model.editor_state.search_pattern.push(c);
            },
        | Msg::EditorSearchBackspace =>
            if model.editor_state.submode == Some(EditorSubMode::Search) {
                model.editor_state.search_pattern.pop();
            },
        | Msg::EditorExecuteSearch =>
            if model.editor_state.submode == Some(EditorSubMode::Search) {
                model.editor_state.execute_search();
                model.editor_state.submode = None;
            },
        | Msg::EditorSearchNext => model.editor_state.search_next(),
        | Msg::EditorSearchPrev => model.editor_state.search_prev(),

        // ===== Ctrl+X 프리픽스 =====
        | Msg::EditorEnterCtrlXMode => model.editor_state.submode = Some(EditorSubMode::CtrlX),
        | Msg::EditorCtrlXSave =>
            if model.editor_state.submode == Some(EditorSubMode::CtrlX) {
                let date = model.editor_state.date;
                let content = model.editor_state.get_content();
                model.editor_state.submode = None;
                return Some(Command::SaveDiary(date, content));
            },
        | Msg::EditorCtrlXBack =>
            if model.editor_state.submode == Some(EditorSubMode::CtrlX) {
                model.screen = crate::app::Screen::Calendar;
                model.editor_state.submode = None;
            },

        | _ => {},
    }

    None
}
