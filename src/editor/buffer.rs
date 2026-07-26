use std::path::PathBuf;

#[derive(Clone)]
pub struct TextBuffer {
    pub text: String,
    saved_text: String,
    pub file_path: Option<PathBuf>,
    pub cursor_offset: usize,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub word_wrap: bool,
    error_msg: String,
}

#[allow(dead_code)]
impl Default for TextBuffer {
    fn default() -> Self {
        Self {
            text: String::new(),
            saved_text: String::new(),
            file_path: None,
            cursor_offset: 0,
            cursor_line: 1,
            cursor_col: 1,
            word_wrap: false,
            error_msg: String::new(),
        }
    }
}

#[allow(dead_code)]
impl TextBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_string(s: String) -> Self {
        let mut buf = Self {
            text: s.clone(),
            saved_text: s,
            ..Default::default()
        };
        buf.recalc_cursor();
        buf
    }

    pub fn from_path(path: &PathBuf) -> Self {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let mut buf = Self {
            text: content.clone(),
            saved_text: content,
            file_path: Some(path.clone()),
            ..Default::default()
        };
        buf.recalc_cursor();
        buf
    }

    pub fn is_dirty(&self) -> bool {
        self.text != self.saved_text
    }

    pub fn error(&self) -> &str {
        &self.error_msg
    }

    pub fn clear_error(&mut self) {
        self.error_msg.clear();
    }

    pub fn save(&mut self) -> Result<(), String> {
        if let Some(path) = &self.file_path {
            std::fs::write(path, &self.text).map_err(|e| format!("Error saving: {}", e))?;
            self.saved_text = self.text.clone();
            self.error_msg.clear();
            Ok(())
        } else {
            Err("No file path set".into())
        }
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        std::fs::write(&path, &self.text).map_err(|e| format!("Error saving: {}", e))?;
        self.file_path = Some(path);
        self.saved_text = self.text.clone();
        self.error_msg.clear();
        Ok(())
    }

    pub fn new_file(&mut self) {
        self.text.clear();
        self.saved_text.clear();
        self.file_path = None;
        self.error_msg.clear();
        self.cursor_offset = 0;
        self.cursor_line = 1;
        self.cursor_col = 1;
    }

    pub fn insert_char(&mut self, ch: char) {
        self.text.insert(self.cursor_offset, ch);
        self.cursor_offset += ch.len_utf8();
        self.recalc_cursor();
    }

    pub fn insert_str(&mut self, s: &str) {
        self.text.insert_str(self.cursor_offset, s);
        self.cursor_offset += s.len();
        self.recalc_cursor();
    }

    pub fn delete_backward(&mut self) -> bool {
        if self.cursor_offset == 0 {
            return false;
        }
        let prev = self.previous_boundary();
        self.text.drain(prev..self.cursor_offset);
        self.cursor_offset = prev;
        self.recalc_cursor();
        true
    }

    pub fn delete_forward(&mut self) -> bool {
        if self.cursor_offset >= self.text.len() {
            return false;
        }
        let next = self.next_boundary();
        self.text.drain(self.cursor_offset..next);
        self.recalc_cursor();
        true
    }

    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub fn cursor_up(&mut self) {
        if self.cursor_line <= 1 {
            self.cursor_offset = 0;
            self.recalc_cursor();
            return;
        }
        let target_col = self.cursor_col;
        let mut line = 1;
        let mut offset = 0;
        for (i, c) in self.text.char_indices() {
            if line >= self.cursor_line - 1 {
                let col = i - offset + 1;
                if col >= target_col || c == '\n' {
                    self.cursor_offset = if c == '\n' { i } else { i };
                    break;
                }
            }
            if c == '\n' {
                line += 1;
                offset = i + 1;
            }
            if i == self.text.len() - 1 {
                self.cursor_offset = self.text.len();
            }
        }
        self.recalc_cursor();
    }

    pub fn cursor_down(&mut self) {
        let target_col = self.cursor_col;
        let mut line = 1;
        let mut offset = 0;
        for (i, c) in self.text.char_indices() {
            if line > self.cursor_line {
                let col = i - offset + 1;
                if col >= target_col || c == '\n' {
                    self.cursor_offset = i;
                    break;
                }
            }
            if c == '\n' {
                line += 1;
                offset = i + 1;
            }
            if i == self.text.len() - 1 {
                self.cursor_offset = self.text.len();
            }
        }
        if self.cursor_offset > self.text.len() {
            self.cursor_offset = self.text.len();
        }
        self.recalc_cursor();
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_offset == 0 {
            return;
        }
        self.cursor_offset = self.previous_boundary();
        self.recalc_cursor();
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_offset >= self.text.len() {
            return;
        }
        self.cursor_offset = self.next_boundary();
        self.recalc_cursor();
    }

    pub fn home(&mut self) {
        let line_start = self.text[..self.cursor_offset]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        self.cursor_offset = line_start;
        self.recalc_cursor();
    }

    pub fn end(&mut self) {
        let line_end = self.text[self.cursor_offset..]
            .find('\n')
            .map(|i| self.cursor_offset + i)
            .unwrap_or(self.text.len());
        self.cursor_offset = line_end;
        self.recalc_cursor();
    }

    pub fn insert_time_date(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let secs = now % 60;
        let mins = (now / 60) % 60;
        let hours = (now / 3600) % 24;
        let day = (now / 86400) % 31 + 1;
        let month = ((now / 86400 / 30) % 12) + 1;
        let year = 2026;
        let date_str = format!(
            "{:02}:{:02}:{:02} {:02}/{:02}/{}",
            hours, mins, secs, day, month, year
        );
        self.insert_str(&date_str);
    }

    pub fn set_cursor(&mut self, byte_offset: usize) {
        self.cursor_offset = byte_offset.min(self.text.len());
        self.recalc_cursor();
    }

    fn previous_boundary(&self) -> usize {
        if self.cursor_offset == 0 {
            return 0;
        }
        let mut boundary = 0;
        for (i, c) in self.text.char_indices() {
            if i >= self.cursor_offset {
                break;
            }
            boundary = i + c.len_utf8();
        }
        if boundary >= self.cursor_offset {
            let mut chars = self.text[..self.cursor_offset].chars();
            chars.next_back();
            self.cursor_offset - chars.next_back().map_or(1, |c| c.len_utf8())
        } else {
            boundary
        }
    }

    fn next_boundary(&self) -> usize {
        if self.cursor_offset >= self.text.len() {
            return self.text.len();
        }
        let remaining = &self.text[self.cursor_offset..];
        self.cursor_offset + remaining.chars().next().map_or(0, |c| c.len_utf8())
    }

    fn recalc_cursor(&mut self) {
        self.cursor_line = 1;
        self.cursor_col = 1;
        let limit = self.cursor_offset.min(self.text.len());
        for c in self.text[..limit].chars() {
            if c == '\n' {
                self.cursor_line += 1;
                self.cursor_col = 1;
            } else {
                self.cursor_col += 1;
            }
        }
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".into())
    }
}
