//! 날짜·UI·저장소와 무관한 순수 멀티라인 텍스트 에디터 엔진.
//!
//! 커서 이동, 편집, 선택(selection), undo/redo, 검색을 제공한다.
//! 외부 의존성이 없으며 어떤 애플리케이션에서도 재사용 가능하다.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub anchor_line: usize,
    pub anchor_col: usize,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

pub struct Snapshot {
    pub content: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub selection: Option<Selection>,
}

pub struct TextEditor {
    pub content: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub is_modified: bool,
    pub selection: Option<Selection>,
    pub edit_history: Vec<Snapshot>,
    pub history_index: usize,
    pub clipboard: String,
    pub search_pattern: String,
    pub search_matches: Vec<(usize, usize)>,
    pub current_match_index: usize,
}

impl Default for TextEditor {
    fn default() -> Self { Self::new() }
}

impl TextEditor {
    pub fn new() -> Self {
        let mut state = Self {
            content: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            is_modified: false,
            selection: None,
            edit_history: Vec::new(),
            history_index: 0,
            clipboard: String::new(),
            search_pattern: String::new(),
            search_matches: Vec::new(),
            current_match_index: 0,
        };

        // 초기 스냅샷 저장
        state.save_snapshot();
        state
    }

    /// 문자 인덱스를 바이트 인덱스로 변환
    pub fn char_idx_to_byte_idx(&self, line: usize, char_idx: usize) -> usize {
        if line >= self.content.len() {
            return 0;
        }

        self.content[line]
            .char_indices()
            .nth(char_idx)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or_else(|| self.content[line].len())
    }

    /// 바이트 인덱스를 문자 인덱스로 변환
    fn byte_idx_to_char_idx(&self, line: usize, byte_idx: usize) -> usize {
        if line >= self.content.len() {
            return 0;
        }

        self.content[line][.. byte_idx.min(self.content[line].len())].chars().count()
    }

    pub fn save_snapshot(&mut self) {
        // 현재 index 이후의 히스토리 제거 (분기된 히스토리 삭제)
        self.edit_history.truncate(self.history_index + 1);

        // 현재 상태 저장
        let snapshot = Snapshot {
            content: self.content.clone(),
            cursor_line: self.cursor_line,
            cursor_col: self.cursor_col,
            selection: self.selection.clone(),
        };

        self.edit_history.push(snapshot);
        self.history_index = self.edit_history.len() - 1;

        // 히스토리 크기 제한 (메모리 관리)
        const MAX_HISTORY: usize = 100;
        if self.edit_history.len() > MAX_HISTORY {
            self.edit_history.drain(0 .. 1);
            self.history_index -= 1;
        }
    }

    fn restore_snapshot(&mut self, index: usize) {
        if let Some(snapshot) = self.edit_history.get(index) {
            self.content = snapshot.content.clone();
            self.cursor_line = snapshot.cursor_line;
            self.cursor_col = snapshot.cursor_col;
            self.selection = snapshot.selection.clone();
            self.history_index = index;
            self.is_modified = true;
        }
    }

    pub fn undo(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.restore_snapshot(self.history_index);
        }
    }

    pub fn redo(&mut self) {
        if self.history_index + 1 < self.edit_history.len() {
            self.history_index += 1;
            self.restore_snapshot(self.history_index);
        }
    }

    pub fn insert_char(&mut self, c: char) {
        if self.cursor_line >= self.content.len() {
            self.content.push(String::new());
        }

        let byte_idx = self.char_idx_to_byte_idx(self.cursor_line, self.cursor_col);
        self.content[self.cursor_line].insert(byte_idx, c);
        self.cursor_col += 1;
        self.is_modified = true;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let byte_idx = self.char_idx_to_byte_idx(self.cursor_line, self.cursor_col - 1);
            self.content[self.cursor_line].remove(byte_idx);
            self.cursor_col -= 1;
            self.is_modified = true;
        } else if self.cursor_line > 0 {
            let current_line = self.content.remove(self.cursor_line);
            self.cursor_line -= 1;
            let line_len = self.content[self.cursor_line].chars().count();
            self.cursor_col = line_len;
            self.content[self.cursor_line].push_str(&current_line);
            self.is_modified = true;
        }
    }

    pub fn delete_forward(&mut self) {
        let line_char_len = if self.cursor_line < self.content.len() {
            self.content[self.cursor_line].chars().count()
        } else {
            return;
        };

        if self.cursor_col < line_char_len {
            // 커서 앞 문자 삭제
            let byte_idx = self.char_idx_to_byte_idx(self.cursor_line, self.cursor_col);
            self.content[self.cursor_line].remove(byte_idx);
            self.is_modified = true;
        } else if self.cursor_line + 1 < self.content.len() {
            // 줄 끝이면 다음 줄을 합침
            let next_line = self.content.remove(self.cursor_line + 1);
            self.content[self.cursor_line].push_str(&next_line);
            self.is_modified = true;
        }
    }

    pub fn kill_line(&mut self) -> String {
        if self.cursor_line >= self.content.len() {
            return String::new();
        }

        let line_char_len = self.content[self.cursor_line].chars().count();

        if self.cursor_col >= line_char_len {
            // 커서가 줄 끝이면 다음 줄을 합침
            if self.cursor_line + 1 < self.content.len() {
                let next_line = self.content.remove(self.cursor_line + 1);
                self.content[self.cursor_line].push_str(&next_line);
                self.is_modified = true;
                return "\n".to_string();
            }
            return String::new();
        }

        // 커서 위치부터 줄 끝까지 잘라내기
        let byte_idx = self.char_idx_to_byte_idx(self.cursor_line, self.cursor_col);
        let killed = self.content[self.cursor_line][byte_idx ..].to_string();
        self.content[self.cursor_line].truncate(byte_idx);
        self.is_modified = true;
        killed
    }

    pub fn new_line(&mut self) {
        let byte_idx = self.char_idx_to_byte_idx(self.cursor_line, self.cursor_col);
        let current_line = &self.content[self.cursor_line];
        let remaining = current_line[byte_idx ..].to_string();
        self.content[self.cursor_line].truncate(byte_idx);

        self.cursor_line += 1;
        self.content.insert(self.cursor_line, remaining);
        self.cursor_col = 0;
        self.is_modified = true;
    }

    pub fn open_line(&mut self) {
        let byte_idx = self.char_idx_to_byte_idx(self.cursor_line, self.cursor_col);
        let current_line = &self.content[self.cursor_line];
        let remaining = current_line[byte_idx ..].to_string();
        self.content[self.cursor_line].truncate(byte_idx);
        self.content.insert(self.cursor_line + 1, remaining);
        self.is_modified = true;
    }

    pub fn load_content(&mut self, content: &str) {
        self.content = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(String::from).collect()
        };
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.is_modified = false;

        // 로드 후 히스토리 초기화
        self.edit_history.clear();
        self.save_snapshot();
    }

    pub fn get_content(&self) -> String { self.content.join("\n") }

    pub fn get_selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        self.selection.as_ref().map(|sel| {
            let start = if sel.anchor_line < sel.cursor_line || (sel.anchor_line == sel.cursor_line && sel.anchor_col < sel.cursor_col) {
                (sel.anchor_line, sel.anchor_col)
            } else {
                (sel.cursor_line, sel.cursor_col)
            };

            let end = if sel.anchor_line > sel.cursor_line || (sel.anchor_line == sel.cursor_line && sel.anchor_col > sel.cursor_col) {
                (sel.anchor_line, sel.anchor_col)
            } else {
                (sel.cursor_line, sel.cursor_col)
            };

            (start, end)
        })
    }

    pub fn get_selected_text(&self) -> Option<String> {
        let ((start_line, start_col), (end_line, end_col)) = self.get_selection_range()?;

        if end_line >= self.content.len() {
            return None;
        }

        if start_line == end_line {
            let line = &self.content[start_line];
            let line_char_len = line.chars().count();
            let safe_end = end_col.min(line_char_len);
            let safe_start = start_col.min(safe_end);

            let start_byte = self.char_idx_to_byte_idx(start_line, safe_start);
            let end_byte = self.char_idx_to_byte_idx(start_line, safe_end);
            Some(line[start_byte .. end_byte].to_string())
        } else {
            let mut result = String::new();

            let start_line_content = &self.content[start_line];
            let start_line_char_len = start_line_content.chars().count();
            let safe_start_col = start_col.min(start_line_char_len);
            let start_byte = self.char_idx_to_byte_idx(start_line, safe_start_col);
            result.push_str(&start_line_content[start_byte ..]);
            result.push('\n');

            for line in (start_line + 1) .. end_line {
                if line < self.content.len() {
                    result.push_str(&self.content[line]);
                    result.push('\n');
                }
            }

            let end_line_content = &self.content[end_line];
            let end_line_char_len = end_line_content.chars().count();
            let safe_end_col = end_col.min(end_line_char_len);
            let end_byte = self.char_idx_to_byte_idx(end_line, safe_end_col);
            result.push_str(&end_line_content[.. end_byte]);

            Some(result)
        }
    }

    pub fn delete_selection(&mut self) {
        let ((start_line, start_col), (end_line, end_col)) = match self.get_selection_range() {
            | Some(range) => range,
            | None => return,
        };

        if start_line == end_line {
            let start_byte = self.char_idx_to_byte_idx(start_line, start_col);
            let end_byte = self.char_idx_to_byte_idx(start_line, end_col);
            self.content[start_line].replace_range(start_byte .. end_byte, "");
            self.cursor_line = start_line;
            self.cursor_col = start_col;
        } else {
            let start_byte = self.char_idx_to_byte_idx(start_line, start_col);
            let end_byte = self.char_idx_to_byte_idx(end_line, end_col);
            let before = self.content[start_line][.. start_byte].to_string();
            let after = self.content[end_line][end_byte ..].to_string();

            self.content.drain(start_line ..= end_line);
            self.content.insert(start_line, before + &after);
            self.cursor_line = start_line;
            self.cursor_col = start_col;
        }

        self.selection = None;
        self.is_modified = true;
    }

    pub fn move_word_next(&mut self) {
        if self.cursor_line >= self.content.len() {
            return;
        }

        let line = &self.content[self.cursor_line];
        let byte_idx = self.char_idx_to_byte_idx(self.cursor_line, self.cursor_col);
        let mut chars = line[byte_idx ..].char_indices().peekable();

        while let Some((_, ch)) = chars.peek() {
            if ch.is_whitespace() {
                break;
            }
            chars.next();
        }

        while let Some((_, ch)) = chars.peek() {
            if !ch.is_whitespace() {
                break;
            }
            chars.next();
        }

        if let Some((idx, _)) = chars.next() {
            self.cursor_col = self.byte_idx_to_char_idx(self.cursor_line, byte_idx + idx);
        } else {
            self.cursor_col = line.chars().count();
        }
    }

    pub fn move_word_prev(&mut self) {
        if self.cursor_line >= self.content.len() || self.cursor_col == 0 {
            return;
        }

        let line = &self.content[self.cursor_line];
        let mut pos = self.cursor_col;

        pos = pos.saturating_sub(1);

        while pos > 0 && line.chars().nth(pos).is_some_and(|c| c.is_whitespace()) {
            pos -= 1;
        }

        while pos > 0 && line.chars().nth(pos - 1).is_some_and(|c| !c.is_whitespace()) {
            pos -= 1;
        }

        self.cursor_col = pos;
    }

    pub fn execute_search(&mut self) {
        if self.search_pattern.is_empty() {
            return;
        }

        self.search_matches.clear();
        for (line_idx, line) in self.content.iter().enumerate() {
            let mut start_byte = 0;
            while let Some(pos_byte) = line[start_byte ..].find(&self.search_pattern) {
                let match_byte = start_byte + pos_byte;
                let match_char = self.byte_idx_to_char_idx(line_idx, match_byte);
                self.search_matches.push((line_idx, match_char));
                start_byte = match_byte + 1;
            }
        }

        if !self.search_matches.is_empty() {
            self.current_match_index = self
                .search_matches
                .iter()
                .position(|(line, col)| *line > self.cursor_line || (*line == self.cursor_line && *col >= self.cursor_col))
                .unwrap_or(0);

            let (line, col) = self.search_matches[self.current_match_index];
            self.cursor_line = line;
            self.cursor_col = col;

            let pattern_char_len = self.search_pattern.chars().count();
            self.selection = Some(Selection {
                anchor_line: line,
                anchor_col: col,
                cursor_line: line,
                cursor_col: col + pattern_char_len,
            });
        }
    }

    pub fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }

        self.current_match_index = (self.current_match_index + 1) % self.search_matches.len();

        let (line, col) = self.search_matches[self.current_match_index];
        self.cursor_line = line;
        self.cursor_col = col;

        let pattern_char_len = self.search_pattern.chars().count();
        self.selection = Some(Selection {
            anchor_line: line,
            anchor_col: col,
            cursor_line: line,
            cursor_col: col + pattern_char_len,
        });
    }

    pub fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }

        if self.current_match_index == 0 {
            self.current_match_index = self.search_matches.len() - 1;
        } else {
            self.current_match_index -= 1;
        }

        let (line, col) = self.search_matches[self.current_match_index];
        self.cursor_line = line;
        self.cursor_col = col;

        let pattern_char_len = self.search_pattern.chars().count();
        self.selection = Some(Selection {
            anchor_line: line,
            anchor_col: col,
            cursor_line: line,
            cursor_col: col + pattern_char_len,
        });
    }
}
