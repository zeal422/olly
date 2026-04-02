use super::App;

impl App {
    fn input_char_len(&self) -> usize {
        self.input.chars().count()
    }

    fn byte_index_for_char(input: &str, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        let total_chars = input.chars().count();
        if char_index >= total_chars {
            return input.len();
        }
        input
            .char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or(input.len())
    }

    fn clamp_input_cursor(&mut self) {
        self.input_cursor_char_index = self.input_cursor_char_index.min(self.input_char_len());
    }

    pub(crate) fn move_input_cursor_left(&mut self) {
        self.clamp_input_cursor();
        self.input_cursor_char_index = self.input_cursor_char_index.saturating_sub(1);
        self.input_cursor_preferred_column = None;
    }

    pub(crate) fn move_input_cursor_right(&mut self) {
        self.clamp_input_cursor();
        self.input_cursor_char_index =
            (self.input_cursor_char_index + 1).min(self.input_char_len());
        self.input_cursor_preferred_column = None;
    }

    fn input_line_count(&self) -> usize {
        self.input.split('\n').count().max(1)
    }

    pub(crate) fn input_line_col_for_cursor(&self) -> (usize, usize) {
        let cursor = self.input_cursor_char_index.min(self.input_char_len());
        let mut line = 0_usize;
        let mut col = 0_usize;
        for (idx, ch) in self.input.chars().enumerate() {
            if idx >= cursor {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn input_char_index_for_line_col(&self, target_line: usize, target_col: usize) -> usize {
        let lines = self.input.split('\n').collect::<Vec<_>>();
        let line_index = target_line.min(lines.len().saturating_sub(1));
        let column = target_col.min(lines[line_index].chars().count());
        let mut char_index = 0_usize;
        for line in lines.iter().take(line_index) {
            char_index += line.chars().count() + 1;
        }
        char_index + column
    }

    pub(crate) fn move_input_cursor_up(&mut self) {
        let (line, col) = self.input_line_col_for_cursor();
        if line == 0 {
            return;
        }
        let target_col = self.input_cursor_preferred_column.unwrap_or(col);
        self.input_cursor_char_index = self.input_char_index_for_line_col(line - 1, target_col);
        self.input_cursor_preferred_column = Some(target_col);
    }

    pub(crate) fn move_input_cursor_down(&mut self) {
        let (line, col) = self.input_line_col_for_cursor();
        if line + 1 >= self.input_line_count() {
            return;
        }
        let target_col = self.input_cursor_preferred_column.unwrap_or(col);
        self.input_cursor_char_index = self.input_char_index_for_line_col(line + 1, target_col);
        self.input_cursor_preferred_column = Some(target_col);
    }

    pub(crate) fn insert_input_char(&mut self, ch: char) {
        self.clear_chat_selection();
        self.clamp_input_cursor();
        let insert_at = Self::byte_index_for_char(&self.input, self.input_cursor_char_index);
        self.input.insert(insert_at, ch);
        self.input_cursor_char_index += 1;
        self.input_cursor_preferred_column = None;
        self.reset_command_selection();
    }

    pub(crate) fn insert_input_string(&mut self, text: String) {
        self.clear_chat_selection();
        self.clamp_input_cursor();
        let insert_at = Self::byte_index_for_char(&self.input, self.input_cursor_char_index);
        self.input.insert_str(insert_at, &text);
        self.input_cursor_char_index += text.chars().count();
        self.input_cursor_preferred_column = None;
        self.reset_command_selection();
    }

    pub(crate) fn backspace_input_char(&mut self) {
        self.clear_chat_selection();
        self.clamp_input_cursor();
        if self.input_cursor_char_index == 0 {
            return;
        }
        let remove_start = Self::byte_index_for_char(&self.input, self.input_cursor_char_index - 1);
        let remove_end = Self::byte_index_for_char(&self.input, self.input_cursor_char_index);
        self.input.drain(remove_start..remove_end);
        self.input_cursor_char_index -= 1;
        self.input_cursor_preferred_column = None;
        self.reset_command_selection();
    }

    pub(crate) fn insert_newline_at_cursor(&mut self) {
        self.insert_input_char('\n');
    }

    pub(crate) fn clear_input_editor_state(&mut self) {
        self.input_cursor_char_index = self.input_char_len();
        self.input_cursor_preferred_column = None;
    }

    pub(crate) fn set_input_text(&mut self, value: String) {
        self.input = value;
        self.clear_input_editor_state();
    }
}
