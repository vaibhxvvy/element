use std::path::PathBuf;

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

impl TextBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(path: &PathBuf) -> Self {
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

    pub fn take_error(&mut self) -> String {
        std::mem::take(&mut self.error_msg)
    }

    pub fn save(&mut self) -> Result<(), String> {
        if let Some(path) = &self.file_path {
            std::fs::write(path, &self.text).map_err(|e| format!("Error saving: {}", e))?;
            self.saved_text = self.text.clone();
            Ok(())
        } else {
            Err("No file path set".into())
        }
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        std::fs::write(&path, &self.text).map_err(|e| format!("Error saving: {}", e))?;
        self.file_path = Some(path);
        self.saved_text = self.text.clone();
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
        self.text.insert_str(self.cursor_offset, &date_str);
        self.cursor_offset += date_str.len();
        self.recalc_cursor();
    }

    pub fn set_cursor(&mut self, byte_offset: usize) {
        self.cursor_offset = byte_offset;
        self.recalc_cursor();
    }

    fn recalc_cursor(&mut self) {
        self.cursor_line = 1;
        self.cursor_col = 1;
        for c in self.text[..self.cursor_offset.min(self.text.len())].chars() {
            if c == '\n' {
                self.cursor_line += 1;
                self.cursor_col = 1;
            } else {
                self.cursor_col += 1;
            }
        }
    }
}
