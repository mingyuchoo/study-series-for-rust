use ratatui_diary::{Model,
                    Msg,
                    model::{EditorSubMode,
                            Screen},
                    storage::Storage};
use std::collections::HashSet;
use tempfile::TempDir;

// ===== 달력 네비게이션 테스트 =====

#[test]
fn test_calendar_move_navigation() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);

    let original_date = model.calendar_state.selected_date;

    ratatui_diary::update::update(&mut model, Msg::CalendarMoveRight);
    assert_ne!(model.calendar_state.selected_date, original_date);

    ratatui_diary::update::update(&mut model, Msg::CalendarMoveLeft);
    assert_eq!(model.calendar_state.selected_date, original_date);
}

#[test]
fn test_calendar_select_date_switches_to_editor() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);

    let cmd = ratatui_diary::update::update(&mut model, Msg::CalendarSelectDate);

    assert_eq!(model.screen, Screen::Editor);
    assert!(cmd.is_some());
}

#[test]
fn test_calendar_month_navigation() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    let original_month = model.calendar_state.current_month;

    ratatui_diary::update::update(&mut model, Msg::CalendarNextMonth);
    let expected = if original_month == 12 { 1 } else { original_month + 1 };
    assert_eq!(model.calendar_state.current_month, expected);
}

#[test]
fn test_calendar_prev_month() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    let original_month = model.calendar_state.current_month;

    ratatui_diary::update::update(&mut model, Msg::CalendarPrevMonth);
    let expected = if original_month == 1 { 12 } else { original_month - 1 };
    assert_eq!(model.calendar_state.current_month, expected);
}

// ===== 에디터 네비게이션 테스트 =====

#[test]
fn test_editor_basic_movement() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["hello world".to_string(), "test".to_string()];
    model.editor_state.cursor_line = 0;
    model.editor_state.cursor_col = 0;

    ratatui_diary::update::update(&mut model, Msg::EditorMoveRight);
    assert_eq!(model.editor_state.cursor_col, 1);

    ratatui_diary::update::update(&mut model, Msg::EditorMoveDown);
    assert_eq!(model.editor_state.cursor_line, 1);
}

#[test]
fn test_editor_word_movement() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["hello world test".to_string()];
    model.editor_state.cursor_col = 0;

    ratatui_diary::update::update(&mut model, Msg::EditorWordNext);
    assert!(model.editor_state.cursor_col > 0);
}

// ===== 에디터 점프 테스트 (Goto 모드 없이 직접 호출) =====

#[test]
fn test_editor_goto_doc_start() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["line1".to_string(), "line2".to_string(), "line3".to_string()];
    model.editor_state.cursor_line = 2;
    model.editor_state.cursor_col = 3;

    ratatui_diary::update::update(&mut model, Msg::EditorGotoDocStart);
    assert_eq!(model.editor_state.cursor_line, 0);
    assert_eq!(model.editor_state.cursor_col, 0);
}

#[test]
fn test_editor_goto_line_end() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["hello world".to_string()];
    model.editor_state.cursor_col = 0;

    ratatui_diary::update::update(&mut model, Msg::EditorGotoLineEnd);
    assert_eq!(model.editor_state.cursor_col, "hello world".len());
}

// ===== 에디터 문자 입력 테스트 (항상 활성) =====

#[test]
fn test_editor_insert_char() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;

    ratatui_diary::update::update(&mut model, Msg::EditorInsertChar('a'));

    assert_eq!(model.editor_state.content[0], "a");
}

#[test]
fn test_editor_open_line() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;

    // "Hello"를 입력한 뒤 커서를 중간(2)에 놓고 open-line
    model.editor_state.content = vec!["Hello".to_string()];
    model.editor_state.cursor_line = 0;
    model.editor_state.cursor_col = 2;

    ratatui_diary::update::update(&mut model, Msg::EditorOpenLine);

    // 줄이 분할되고 커서는 원래 위치에 유지
    assert_eq!(model.editor_state.content.len(), 2);
    assert_eq!(model.editor_state.content[0], "He");
    assert_eq!(model.editor_state.content[1], "llo");
    assert_eq!(model.editor_state.cursor_line, 0);
    assert_eq!(model.editor_state.cursor_col, 2);
}

// ===== 에디터 Selection 테스트 =====

#[test]
fn test_editor_toggle_selection() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;

    ratatui_diary::update::update(&mut model, Msg::EditorToggleSelection);
    assert!(model.editor_state.selection.is_some());

    ratatui_diary::update::update(&mut model, Msg::EditorToggleSelection);
    assert!(model.editor_state.selection.is_none());
}

#[test]
fn test_editor_select_line() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["hello".to_string()];

    ratatui_diary::update::update(&mut model, Msg::EditorSelectLine);
    assert!(model.editor_state.selection.is_some());
    let sel = model.editor_state.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(sel.cursor_col, 5);
}

// ===== 에디터 편집 기능 테스트 =====

#[test]
fn test_editor_delete_with_selection() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["hello world".to_string()];
    model.editor_state.cursor_col = 0;

    // 먼저 줄 선택
    ratatui_diary::update::update(&mut model, Msg::EditorSelectLine);
    ratatui_diary::update::update(&mut model, Msg::EditorDelete);

    // Selection이 삭제 후 해제됨
    assert!(model.editor_state.selection.is_none());
}

#[test]
fn test_editor_delete_forward() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["hello".to_string()];
    model.editor_state.cursor_col = 0;

    ratatui_diary::update::update(&mut model, Msg::EditorDeleteForward);
    assert_eq!(model.editor_state.content[0], "ello");
}

#[test]
fn test_editor_kill_line() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["hello world".to_string()];
    model.editor_state.cursor_col = 5;

    ratatui_diary::update::update(&mut model, Msg::EditorKillLine);
    assert_eq!(model.editor_state.content[0], "hello");
    assert_eq!(model.editor_state.clipboard, " world");
}

#[test]
fn test_editor_yank_and_paste() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["hello".to_string()];

    ratatui_diary::update::update(&mut model, Msg::EditorSelectLine);
    ratatui_diary::update::update(&mut model, Msg::EditorYank);

    assert!(!model.editor_state.clipboard.is_empty());
}

#[test]
fn test_editor_paste() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["Hello".to_string()];
    model.editor_state.clipboard = "World".to_string();
    model.editor_state.cursor_col = 5;

    ratatui_diary::update::update(&mut model, Msg::EditorPaste);

    assert!(model.editor_state.content[0].contains("World"));
}

// ===== 에디터 Undo/Redo 테스트 =====

#[test]
fn test_editor_undo_redo() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;

    // 변경
    ratatui_diary::update::update(&mut model, Msg::EditorInsertChar('a'));
    let content_after = model.editor_state.content.clone();

    // Undo
    ratatui_diary::update::update(&mut model, Msg::EditorUndo);

    // Redo
    ratatui_diary::update::update(&mut model, Msg::EditorRedo);
    assert_eq!(model.editor_state.content, content_after);
}

// ===== 에디터 검색 테스트 =====

#[test]
fn test_editor_search_mode() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;

    ratatui_diary::update::update(&mut model, Msg::EditorEnterSearchMode);
    assert_eq!(model.editor_state.submode, Some(EditorSubMode::Search));
}

#[test]
fn test_editor_search_execution() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.content = vec!["hello world".to_string(), "hello test".to_string()];
    model.editor_state.submode = Some(EditorSubMode::Search);
    model.editor_state.search_pattern = "hello".to_string();

    ratatui_diary::update::update(&mut model, Msg::EditorExecuteSearch);
    assert!(!model.editor_state.search_matches.is_empty());
}

// ===== 에디터 Ctrl+X 명령 테스트 =====

#[test]
fn test_editor_ctrl_x_mode() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;

    ratatui_diary::update::update(&mut model, Msg::EditorEnterCtrlXMode);
    assert_eq!(model.editor_state.submode, Some(EditorSubMode::CtrlX));
}

#[test]
fn test_editor_ctrl_x_save() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.submode = Some(EditorSubMode::CtrlX);
    model.editor_state.content = vec!["test".to_string()];

    let cmd = ratatui_diary::update::update(&mut model, Msg::EditorCtrlXSave);
    assert!(cmd.is_some());
    assert_eq!(model.editor_state.submode, None);
}

#[test]
fn test_editor_ctrl_x_back() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.submode = Some(EditorSubMode::CtrlX);

    ratatui_diary::update::update(&mut model, Msg::EditorCtrlXBack);
    assert_eq!(model.screen, Screen::Calendar);
}

// ===== 파일 I/O 결과 테스트 =====

#[test]
fn test_load_diary_success() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;

    let date = model.editor_state.date;
    let content = "Test content\nLine 2".to_string();

    ratatui_diary::update::update(&mut model, Msg::LoadDiarySuccess(date, content));
    assert_eq!(model.editor_state.content.len(), 2);
    assert_eq!(model.editor_state.content[0], "Test content");
}

#[test]
fn test_save_diary_success() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.screen = Screen::Editor;
    model.editor_state.is_modified = true;

    ratatui_diary::update::update(&mut model, Msg::SaveDiarySuccess);
    assert!(!model.editor_state.is_modified);
}

#[test]
fn test_dismiss_error() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let mut model = Model::new(HashSet::new(), storage);
    model.show_error_popup = true;
    model.error_message = Some("Test error".to_string());

    ratatui_diary::update::update(&mut model, Msg::DismissError);
    assert!(!model.show_error_popup);
    assert!(model.error_message.is_none());
}

#[cfg(test)]
mod complete_message_coverage {
    use chrono::NaiveDate;
    use ratatui_diary::{Model,
                        Msg,
                        model::{EditorSubMode,
                                Screen},
                        storage::Storage};
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn setup_model() -> (Model, TempDir) {
        let temp = TempDir::new().unwrap();
        let storage = Storage::with_dir(temp.path()).unwrap();
        let model = Model::new(HashSet::new(), storage);
        (model, temp)
    }

    // ===== 에디터 점프 테스트 =====

    #[test]
    fn test_editor_goto_doc_end() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["Line 1".to_string(), "Line 2".to_string(), "Line 3".to_string()];
        model.editor_state.cursor_line = 0;

        ratatui_diary::update::update(&mut model, Msg::EditorGotoDocEnd);

        assert_eq!(model.editor_state.cursor_line, 2);
    }

    #[test]
    fn test_editor_goto_line_start() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["Hello World".to_string()];
        model.editor_state.cursor_col = 5;

        ratatui_diary::update::update(&mut model, Msg::EditorGotoLineStart);

        assert_eq!(model.editor_state.cursor_col, 0);
    }

    // ===== 에디터 편집 기능 =====

    #[test]
    fn test_editor_delete_without_selection() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["Hello World".to_string()];
        model.editor_state.cursor_line = 0;
        model.editor_state.cursor_col = 0;

        // Selection이 없으면 EditorDelete는 아무것도 하지 않음 (Emacs 스타일)
        ratatui_diary::update::update(&mut model, Msg::EditorDelete);

        assert_eq!(model.editor_state.content[0], "Hello World");
    }

    #[test]
    fn test_editor_kill_line_at_end() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["Hello".to_string(), "World".to_string()];
        model.editor_state.cursor_col = 5; // 줄 끝

        ratatui_diary::update::update(&mut model, Msg::EditorKillLine);

        // 줄 끝에서 kill-line은 다음 줄을 합침
        assert_eq!(model.editor_state.content.len(), 1);
        assert_eq!(model.editor_state.content[0], "HelloWorld");
    }

    #[test]
    fn test_editor_delete_forward_at_end() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["Hello".to_string(), "World".to_string()];
        model.editor_state.cursor_col = 5; // 줄 끝

        ratatui_diary::update::update(&mut model, Msg::EditorDeleteForward);

        // 줄 끝에서 delete-forward는 다음 줄을 합침
        assert_eq!(model.editor_state.content.len(), 1);
        assert_eq!(model.editor_state.content[0], "HelloWorld");
    }

    // ===== 에디터 검색 =====

    #[test]
    fn test_editor_search_char_input() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.submode = Some(EditorSubMode::Search);

        ratatui_diary::update::update(&mut model, Msg::EditorSearchChar('t'));

        assert_eq!(model.editor_state.search_pattern, "t");
    }

    #[test]
    fn test_editor_search_backspace() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.submode = Some(EditorSubMode::Search);
        model.editor_state.search_pattern = "test".to_string();

        ratatui_diary::update::update(&mut model, Msg::EditorSearchBackspace);

        assert_eq!(model.editor_state.search_pattern, "tes");
    }

    #[test]
    fn test_editor_search_prev() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["test test test".to_string()];
        model.editor_state.search_pattern = "test".to_string();
        model.editor_state.execute_search();

        ratatui_diary::update::update(&mut model, Msg::EditorSearchPrev);

        assert!(model.editor_state.current_match_index < 3);
    }

    // ===== 달력 년도 이동 =====

    #[test]
    fn test_calendar_next_year() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Calendar;
        let original_year = model.calendar_state.current_year;

        ratatui_diary::update::update(&mut model, Msg::CalendarNextYear);

        assert_eq!(model.calendar_state.current_year, original_year + 1);
    }

    #[test]
    fn test_calendar_prev_year() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Calendar;
        let original_year = model.calendar_state.current_year;

        ratatui_diary::update::update(&mut model, Msg::CalendarPrevYear);

        assert_eq!(model.calendar_state.current_year, original_year - 1);
    }

    // ===== 파일 I/O =====

    #[test]
    fn test_load_diary_failed() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;

        ratatui_diary::update::update(&mut model, Msg::LoadDiaryFailed("File not found".to_string()));

        assert!(model.show_error_popup);
        assert!(model.error_message.unwrap().contains("File not found"));
    }

    #[test]
    fn test_save_diary_failed() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;

        ratatui_diary::update::update(&mut model, Msg::SaveDiaryFailed("Permission denied".to_string()));

        assert!(model.show_error_popup);
        assert!(model.error_message.unwrap().contains("Permission denied"));
    }

    #[test]
    fn test_delete_diary_success() {
        let (mut model, _temp) = setup_model();
        let date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        model.diary_entries.entries.insert(date);

        ratatui_diary::update::update(&mut model, Msg::DeleteDiarySuccess(date));

        assert!(!model.diary_entries.entries.contains(&date));
    }

    #[test]
    fn test_refresh_index() {
        let (mut model, _temp) = setup_model();
        let date1 = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let date2 = NaiveDate::from_ymd_opt(2026, 2, 16).unwrap();
        let mut new_entries = HashSet::new();
        new_entries.insert(date1);
        new_entries.insert(date2);

        ratatui_diary::update::update(&mut model, Msg::RefreshIndex(new_entries));

        assert_eq!(model.diary_entries.entries.len(), 2);
    }

    // ===== Helper Function Tests =====

    #[test]
    fn test_paste_clipboard_line_mode() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["Line 1".to_string()];
        model.editor_state.clipboard = "New Line\n".to_string();
        model.editor_state.cursor_line = 0;

        ratatui_diary::update::update(&mut model, Msg::EditorPaste);

        assert!(model.editor_state.content.len() >= 2);
    }

    #[test]
    fn test_paste_clipboard_char_mode() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["Hello".to_string()];
        model.editor_state.clipboard = "World".to_string();
        model.editor_state.cursor_col = 5;

        ratatui_diary::update::update(&mut model, Msg::EditorPaste);

        assert!(model.editor_state.content[0].contains("World"));
    }

    // ===== 기타 메시지 =====

    #[test]
    fn test_quit_message() {
        let (mut model, _temp) = setup_model();
        let cmd = ratatui_diary::update::update(&mut model, Msg::Quit);
        assert!(cmd.is_none());
    }

    #[test]
    fn test_calendar_move_up() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Calendar;
        let original_date = model.calendar_state.selected_date;

        ratatui_diary::update::update(&mut model, Msg::CalendarMoveUp);

        assert!(model.calendar_state.selected_date < original_date);
    }

    #[test]
    fn test_calendar_move_down() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Calendar;
        let original_date = model.calendar_state.selected_date;

        ratatui_diary::update::update(&mut model, Msg::CalendarMoveDown);

        assert!(model.calendar_state.selected_date > original_date);
    }

    #[test]
    fn test_editor_move_left() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["Hello".to_string()];
        model.editor_state.cursor_col = 3;

        ratatui_diary::update::update(&mut model, Msg::EditorMoveLeft);

        assert_eq!(model.editor_state.cursor_col, 2);
    }

    #[test]
    fn test_editor_move_up() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["Line 1".to_string(), "Line 2".to_string()];
        model.editor_state.cursor_line = 1;

        ratatui_diary::update::update(&mut model, Msg::EditorMoveUp);

        assert_eq!(model.editor_state.cursor_line, 0);
    }

    #[test]
    fn test_editor_word_prev() {
        let (mut model, _temp) = setup_model();
        model.screen = Screen::Editor;
        model.editor_state.content = vec!["hello world test".to_string()];
        model.editor_state.cursor_col = 12;

        ratatui_diary::update::update(&mut model, Msg::EditorWordPrev);

        assert!(model.editor_state.cursor_col < 12);
    }
}
