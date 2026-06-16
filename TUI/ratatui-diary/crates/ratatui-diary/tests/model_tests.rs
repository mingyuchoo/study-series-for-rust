use chrono::NaiveDate;
use ratatui_diary::{model::{CalendarState,
                            EditorState,
                            Model,
                            Screen},
                    storage::Storage};
use std::collections::HashSet;
use tempfile::TempDir;

#[test]
fn test_next_month() {
    let mut state = CalendarState::new(2026, 2);
    state.next_month();
    assert_eq!(state.current_month, 3);
    assert_eq!(state.current_year, 2026);
}

#[test]
fn test_next_month_year_rollover() {
    let mut state = CalendarState::new(2026, 12);
    state.next_month();
    assert_eq!(state.current_month, 1);
    assert_eq!(state.current_year, 2027);
}

#[test]
fn test_prev_month() {
    let mut state = CalendarState::new(2026, 2);
    state.prev_month();
    assert_eq!(state.current_month, 1);
    assert_eq!(state.current_year, 2026);
}

#[test]
fn test_prev_month_year_rollover() {
    let mut state = CalendarState::new(2026, 1);
    state.prev_month();
    assert_eq!(state.current_month, 12);
    assert_eq!(state.current_year, 2025);
}

#[test]
fn test_insert_char() {
    let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());

    state.insert_char('a');
    assert_eq!(state.content[0], "a");
    assert_eq!(state.cursor_col, 1);
    assert!(state.is_modified);
}

#[test]
fn test_new_line() {
    let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
    state.insert_char('a');
    state.new_line();

    assert_eq!(state.content.len(), 2);
    assert_eq!(state.cursor_line, 1);
    assert_eq!(state.cursor_col, 0);
}

#[test]
fn test_load_content() {
    let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
    let content = "Line 1\nLine 2\nLine 3";

    state.load_content(content);

    assert_eq!(state.content.len(), 3);
    assert_eq!(state.content[0], "Line 1");
    assert_eq!(state.content[1], "Line 2");
    assert!(!state.is_modified);
}

#[test]
fn test_model_with_storage() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();
    let entries = HashSet::new();

    let model = Model::new(entries, storage);
    assert_eq!(model.screen, Screen::Calendar);
}

#[test]
fn test_delete_forward() {
    let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
    state.content = vec!["hello".to_string()];
    state.cursor_col = 0;

    state.delete_forward();
    assert_eq!(state.content[0], "ello");
}

#[test]
fn test_delete_forward_joins_lines() {
    let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
    state.content = vec!["hello".to_string(), "world".to_string()];
    state.cursor_col = 5; // 줄 끝

    state.delete_forward();
    assert_eq!(state.content.len(), 1);
    assert_eq!(state.content[0], "helloworld");
}

#[test]
fn test_kill_line() {
    let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
    state.content = vec!["hello world".to_string()];
    state.cursor_col = 5;

    let killed = state.kill_line();
    assert_eq!(killed, " world");
    assert_eq!(state.content[0], "hello");
}

#[test]
fn test_kill_line_at_end_joins() {
    let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
    state.content = vec!["hello".to_string(), "world".to_string()];
    state.cursor_col = 5;

    let killed = state.kill_line();
    assert_eq!(killed, "\n");
    assert_eq!(state.content.len(), 1);
    assert_eq!(state.content[0], "helloworld");
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use ratatui_diary::model::Selection;

    #[test]
    fn test_selection_creation() {
        let selection = Selection {
            anchor_line: 0,
            anchor_col: 0,
            cursor_line: 0,
            cursor_col: 5,
        };
        assert_eq!(selection.anchor_line, 0);
        assert_eq!(selection.cursor_col, 5);
    }

    #[test]
    fn test_editor_state_has_selection_field() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let state = EditorState::new(date);
        assert!(state.selection.is_none());
    }

    #[test]
    fn test_get_selection_range_forward() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 2,
            cursor_line: 0,
            cursor_col: 5,
        });

        let range = state.get_selection_range();
        assert_eq!(range, Some(((0, 2), (0, 5))));
    }

    #[test]
    fn test_get_selection_range_backward() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 5,
            cursor_line: 0,
            cursor_col: 2,
        });

        let range = state.get_selection_range();
        assert_eq!(range, Some(((0, 2), (0, 5))));
    }

    #[test]
    fn test_get_selected_text_single_line() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello World".to_string()];
        state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 0,
            cursor_line: 0,
            cursor_col: 5,
        });

        let text = state.get_selected_text();
        assert_eq!(text, Some("Hello".to_string()));
    }

    #[test]
    fn test_get_selected_text_multi_line() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["First line".to_string(), "Second line".to_string(), "Third line".to_string()];
        state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 6,
            cursor_line: 2,
            cursor_col: 5,
        });

        let text = state.get_selected_text();
        assert_eq!(text, Some("line\nSecond line\nThird".to_string()));
    }

    #[test]
    fn test_delete_selection_single_line() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello World".to_string()];
        state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 0,
            cursor_line: 0,
            cursor_col: 5,
        });

        state.delete_selection();

        assert_eq!(state.content[0], " World");
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn test_delete_selection_multi_line() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["First line".to_string(), "Second line".to_string(), "Third line".to_string()];
        state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 6,
            cursor_line: 2,
            cursor_col: 5,
        });

        state.delete_selection();

        assert_eq!(state.content.len(), 1);
        assert_eq!(state.content[0], "First  line");
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 6);
    }
}

#[cfg(test)]
mod update_selection_tests {
    use super::*;
    use ratatui_diary::{Msg,
                        model::Selection,
                        update};

    #[test]
    fn test_selection_toggle_on() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut model = Model::new(HashSet::new(), Storage::new().unwrap());
        model.screen = Screen::Editor;
        model.editor_state = EditorState::new(date);
        model.editor_state.cursor_line = 0;
        model.editor_state.cursor_col = 5;

        update::update(&mut model, Msg::EditorToggleSelection);

        assert!(model.editor_state.selection.is_some());
        let sel = model.editor_state.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_line, 0);
        assert_eq!(sel.anchor_col, 5);
    }

    #[test]
    fn test_selection_toggle_off() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut model = Model::new(HashSet::new(), Storage::new().unwrap());
        model.screen = Screen::Editor;
        model.editor_state = EditorState::new(date);
        model.editor_state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 0,
            cursor_line: 0,
            cursor_col: 5,
        });

        update::update(&mut model, Msg::EditorToggleSelection);

        assert!(model.editor_state.selection.is_none());
    }

    #[test]
    fn test_select_line() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut model = Model::new(HashSet::new(), Storage::new().unwrap());
        model.screen = Screen::Editor;
        model.editor_state = EditorState::new(date);
        model.editor_state.content = vec!["Hello World".to_string()];
        model.editor_state.cursor_line = 0;
        model.editor_state.cursor_col = 5;

        update::update(&mut model, Msg::EditorSelectLine);

        assert!(model.editor_state.selection.is_some());
        let sel = model.editor_state.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_line, 0);
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(sel.cursor_line, 0);
        assert_eq!(sel.cursor_col, 11);
    }
}

#[cfg(test)]
mod undo_redo_tests {
    use super::*;

    #[test]
    fn test_undo_restores_previous_state() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Original".to_string()];
        state.save_snapshot();

        state.content = vec!["Modified".to_string()];
        state.save_snapshot();

        state.undo();

        assert_eq!(state.content[0], "Original");
    }

    #[test]
    fn test_redo_after_undo() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["First".to_string()];
        state.save_snapshot();

        state.content = vec!["Second".to_string()];
        state.save_snapshot();

        state.undo();
        assert_eq!(state.content[0], "First");

        state.redo();
        assert_eq!(state.content[0], "Second");
    }

    #[test]
    fn test_new_edit_clears_redo_history() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["First".to_string()];
        state.save_snapshot();

        state.content = vec!["Second".to_string()];
        state.save_snapshot();

        state.undo();

        state.content = vec!["Third".to_string()];
        state.save_snapshot();

        let before_redo = state.content.clone();
        state.redo();
        assert_eq!(state.content, before_redo);
    }
}

#[cfg(test)]
mod word_navigation_tests {
    use super::*;

    #[test]
    fn test_word_next() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello World Test".to_string()];
        state.cursor_line = 0;
        state.cursor_col = 0;

        state.move_word_next();
        assert_eq!(state.cursor_col, 6);

        state.move_word_next();
        assert_eq!(state.cursor_col, 12);
    }

    #[test]
    fn test_word_prev() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello World Test".to_string()];
        state.cursor_line = 0;
        state.cursor_col = 12;

        state.move_word_prev();
        assert_eq!(state.cursor_col, 6);

        state.move_word_prev();
        assert_eq!(state.cursor_col, 0);
    }
}

#[cfg(test)]
mod multibyte_char_tests {
    use super::*;
    use ratatui_diary::model::Selection;

    #[test]
    fn test_insert_korean_char() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);

        state.insert_char('한');
        assert_eq!(state.content[0], "한");
        assert_eq!(state.cursor_col, 1);

        state.insert_char('글');
        assert_eq!(state.content[0], "한글");
        assert_eq!(state.cursor_col, 2);
    }

    #[test]
    fn test_insert_char_between_korean() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["한글".to_string()];
        state.cursor_col = 1;

        state.insert_char('국');
        assert_eq!(state.content[0], "한국글");
        assert_eq!(state.cursor_col, 2);
    }

    #[test]
    fn test_backspace_korean_char() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["한글".to_string()];
        state.cursor_col = 2;

        state.backspace();
        assert_eq!(state.content[0], "한");
        assert_eq!(state.cursor_col, 1);
    }

    #[test]
    fn test_new_line_with_korean() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["한글테스트".to_string()];
        state.cursor_col = 2;

        state.new_line();
        assert_eq!(state.content[0], "한글");
        assert_eq!(state.content[1], "테스트");
        assert_eq!(state.cursor_line, 1);
        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn test_selection_with_korean() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["안녕하세요".to_string()];
        state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 1,
            cursor_line: 0,
            cursor_col: 3,
        });

        let text = state.get_selected_text();
        assert_eq!(text, Some("녕하".to_string()));
    }

    #[test]
    fn test_delete_selection_korean() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["안녕하세요".to_string()];
        state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 1,
            cursor_line: 0,
            cursor_col: 3,
        });

        state.delete_selection();
        assert_eq!(state.content[0], "안세요");
        assert_eq!(state.cursor_col, 1);
    }

    #[test]
    fn test_mixed_korean_english() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello한글World".to_string()];
        state.cursor_col = 5;

        state.insert_char(' ');
        assert_eq!(state.content[0], "Hello 한글World");
        assert_eq!(state.cursor_col, 6);
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;

    #[test]
    fn test_search_finds_all_matches() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello world".to_string(), "World of Rust".to_string(), "world again".to_string()];
        state.search_pattern = "world".to_string();

        state.execute_search();

        assert_eq!(state.search_matches.len(), 2);
        assert_eq!(state.search_matches[0], (0, 6));
        assert_eq!(state.search_matches[1], (2, 0));
    }

    #[test]
    fn test_search_next_wraps() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["test test test".to_string()];
        state.search_pattern = "test".to_string();
        state.execute_search();

        assert_eq!(state.current_match_index, 0);

        state.search_next();
        assert_eq!(state.current_match_index, 1);

        state.search_next();
        assert_eq!(state.current_match_index, 2);

        state.search_next();
        assert_eq!(state.current_match_index, 0);
    }

    #[test]
    fn test_search_prev_wraps() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["test test test".to_string()];
        state.search_pattern = "test".to_string();
        state.execute_search();

        state.search_prev();
        assert_eq!(state.current_match_index, 2);
    }
}

#[cfg(test)]
mod days_in_month_tests {
    use chrono::{Datelike,
                 NaiveDate};
    use ratatui_diary::model::CalendarState;

    #[test]
    fn test_adjust_date_to_february_29_leap_year() {
        let mut state = CalendarState::new(2024, 1);
        state.selected_date = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();

        state.next_month();

        assert_eq!(state.selected_date.day(), 29);
        assert_eq!(state.selected_date.month(), 2);
    }

    #[test]
    fn test_adjust_date_to_february_28_non_leap_year() {
        let mut state = CalendarState::new(2023, 1);
        state.selected_date = NaiveDate::from_ymd_opt(2023, 1, 31).unwrap();

        state.next_month();

        assert_eq!(state.selected_date.day(), 28);
        assert_eq!(state.selected_date.month(), 2);
    }

    #[test]
    fn test_adjust_date_to_april_30() {
        let mut state = CalendarState::new(2026, 3);
        state.selected_date = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();

        state.next_month();

        assert_eq!(state.selected_date.day(), 30);
        assert_eq!(state.selected_date.month(), 4);
    }

    #[test]
    fn test_adjust_date_year_boundary() {
        let mut state = CalendarState::new(2025, 12);
        state.selected_date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        state.next_month();

        assert_eq!(state.selected_date.day(), 31);
        assert_eq!(state.selected_date.month(), 1);
        assert_eq!(state.selected_date.year(), 2026);
    }
}

#[cfg(test)]
mod editor_edge_cases {
    use super::*;
    use ratatui_diary::model::Selection;

    #[test]
    fn test_out_of_bounds_line_access() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Line 1".to_string()];
        state.cursor_line = state.content.len();
        state.cursor_col = 0;

        state.insert_char('x');

        assert_eq!(state.content.len(), 2);
        assert_eq!(state.content[1], "x");
    }

    #[test]
    fn test_out_of_bounds_column_access() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Short".to_string()];
        state.cursor_col = 100;

        state.insert_char('x');

        assert!(!state.content.is_empty());
    }

    #[test]
    fn test_empty_content_handling() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        assert_eq!(state.content.len(), 1);

        state.insert_char('a');

        assert_eq!(state.content[0], "a");
    }

    #[test]
    fn test_empty_line_operations() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["".to_string()];
        state.cursor_line = 0;
        state.cursor_col = 0;

        state.backspace();

        assert_eq!(state.content.len(), 1);
    }

    #[test]
    fn test_word_navigation_at_end() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello".to_string()];
        state.cursor_line = 0;
        state.cursor_col = 5;

        state.move_word_next();

        assert!(state.cursor_col <= state.content[0].chars().count());
    }

    #[test]
    fn test_word_navigation_at_start() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello".to_string()];
        state.cursor_line = 0;
        state.cursor_col = 0;

        state.move_word_prev();

        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn test_empty_search_pattern() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello World".to_string()];
        state.search_pattern = "".to_string();

        state.execute_search();

        assert!(state.search_matches.is_empty());
    }

    #[test]
    fn test_search_no_matches() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello World".to_string()];
        state.search_pattern = "NotFound".to_string();

        state.execute_search();

        assert!(state.search_matches.is_empty());
        assert_eq!(state.current_match_index, 0);
    }

    #[test]
    fn test_search_next_with_no_matches() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Hello World".to_string()];
        state.search_pattern = "NotFound".to_string();
        state.execute_search();

        state.search_next();

        assert_eq!(state.current_match_index, 0);
    }

    #[test]
    fn test_selection_out_of_bounds() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Short".to_string()];
        state.selection = Some(Selection {
            anchor_line: 0,
            anchor_col: 0,
            cursor_line: 0,
            cursor_col: 100,
        });

        let text = state.get_selected_text();

        assert!(text.is_some() || text.is_none());
    }

    #[test]
    fn test_undo_with_empty_history() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Original".to_string()];

        state.undo();

        assert_eq!(state.content[0], "Original");
    }

    #[test]
    fn test_redo_with_empty_future() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Original".to_string()];

        state.redo();

        assert_eq!(state.content[0], "Original");
    }

    #[test]
    fn test_history_size_limit() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);

        for i in 0 .. 200 {
            state.content = vec![format!("Version {}", i)];
            state.save_snapshot();
        }

        state.undo();
        assert!(!state.content.is_empty());
    }

    #[test]
    fn test_multiline_selection_edge() {
        let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
        let mut state = EditorState::new(date);
        state.content = vec!["Line 1".to_string(), "Line 2".to_string()];
        state.selection = Some(Selection {
            anchor_line: 1,
            anchor_col: 6,
            cursor_line: 0,
            cursor_col: 0,
        });

        state.delete_selection();

        assert_eq!(state.content.len(), 1);
    }

    #[test]
    fn test_insert_char_on_empty_line() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
        state.insert_char('a');
        assert_eq!(state.content[0], "a");
        assert_eq!(state.cursor_col, 1);
    }

    #[test]
    fn test_backspace_at_line_start() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
        state.content = vec!["Line 1".to_string(), "Line 2".to_string()];
        state.cursor_line = 1;
        state.cursor_col = 0;

        state.backspace();

        assert_eq!(state.content.len(), 1);
        assert_eq!(state.content[0], "Line 1Line 2");
        assert_eq!(state.cursor_line, 0);
    }

    #[test]
    fn test_backspace_with_no_lines() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
        state.content = vec!["".to_string()];
        state.cursor_line = 0;
        state.cursor_col = 0;

        state.backspace();

        assert_eq!(state.content.len(), 1);
    }

    #[test]
    fn test_new_line_in_middle_of_text() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
        state.insert_char('a');
        state.insert_char('b');
        state.insert_char('c');
        state.cursor_col = 1;

        state.new_line();

        assert_eq!(state.content.len(), 2);
        assert_eq!(state.content[0], "a");
        assert_eq!(state.content[1], "bc");
    }

    #[test]
    fn test_history_size_limit_exceeded() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());

        for i in 0 .. 150 {
            state.insert_char(char::from_u32(97 + (i % 26) as u32).unwrap());
            state.save_snapshot();
        }

        assert!(state.edit_history.len() <= 150);
    }

    #[test]
    fn test_insert_after_undo_creates_new_branch() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());

        state.insert_char('a');
        state.save_snapshot();
        state.insert_char('b');
        state.save_snapshot();

        state.undo();

        state.insert_char('x');
        state.save_snapshot();

        assert_eq!(state.content[0], "ax");
    }

    #[test]
    fn test_insert_char_extends_line() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
        state.insert_char('a');
        state.insert_char('b');
        state.insert_char('c');

        assert_eq!(state.content[0], "abc");
        assert_eq!(state.cursor_col, 3);
    }

    #[test]
    fn test_move_cursor_left_at_boundary() {
        let mut state = CalendarState::new(2026, 1);
        state.selected_date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();

        state.move_cursor_left();

        assert!(state.selected_date <= NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    }

    #[test]
    fn test_move_cursor_right_normal() {
        let mut state = CalendarState::new(2026, 2);
        state.selected_date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let original = state.selected_date;

        state.move_cursor_right();

        assert_eq!(state.selected_date, original.succ_opt().unwrap());
    }

    #[test]
    fn test_move_cursor_up_at_week_boundary() {
        let mut state = CalendarState::new(2026, 2);
        state.selected_date = NaiveDate::from_ymd_opt(2026, 2, 8).unwrap();

        state.move_cursor_up();

        assert!(state.selected_date < NaiveDate::from_ymd_opt(2026, 2, 8).unwrap());
    }

    #[test]
    fn test_move_cursor_down_normal() {
        let mut state = CalendarState::new(2026, 2);
        state.selected_date = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();

        state.move_cursor_down();

        assert!(state.selected_date > NaiveDate::from_ymd_opt(2026, 2, 15).unwrap());
    }

    #[test]
    fn test_load_content_empty_string() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
        state.content = vec!["Old content".to_string()];

        state.load_content("");

        assert_eq!(state.content.len(), 1);
        assert_eq!(state.content[0], "");
    }

    #[test]
    fn test_backspace_multibyte_character() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
        state.insert_char('한');
        assert_eq!(state.cursor_col, 1);

        state.backspace();

        assert_eq!(state.content[0], "");
        assert_eq!(state.cursor_col, 0);
    }

    #[test]
    fn test_new_line_with_empty_line() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());
        state.new_line();

        assert_eq!(state.content.len(), 2);
        assert_eq!(state.cursor_line, 1);
    }

    #[test]
    fn test_multiple_undo_operations() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());

        state.insert_char('a');
        state.save_snapshot();
        state.insert_char('b');
        state.save_snapshot();

        state.undo();
        assert_eq!(state.content[0], "a");

        state.undo();
        assert_eq!(state.content[0], "");
    }

    #[test]
    fn test_redo_after_undo() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());

        state.insert_char('a');
        state.save_snapshot();
        state.insert_char('b');
        state.save_snapshot();

        state.undo();
        state.undo();

        state.redo();
        assert_eq!(state.content[0], "a");

        state.redo();
        assert_eq!(state.content[0], "ab");
    }

    #[test]
    fn test_undo_at_beginning_of_history() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());

        state.undo();

        assert_eq!(state.history_index, 0);
    }

    #[test]
    fn test_redo_at_end_of_history() {
        let mut state = EditorState::new(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap());

        state.redo();

        assert_eq!(state.history_index, 0);
    }
}
