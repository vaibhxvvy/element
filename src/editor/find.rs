use super::buffer::TextBuffer;

#[derive(Clone)]
pub struct FindMatch {
    pub start: usize,
    pub end: usize,
}

pub struct FindState {
    pub query: String,
    pub match_count: usize,
    pub current_match: usize,
    pub matches: Vec<FindMatch>,
    last_pos: Option<usize>,
}

impl Default for FindState {
    fn default() -> Self {
        Self {
            query: String::new(),
            match_count: 0,
            current_match: 0,
            matches: Vec::new(),
            last_pos: None,
        }
    }
}

impl FindState {
    pub fn search(&mut self, buffer: &TextBuffer) {
        self.matches.clear();
        self.match_count = 0;
        self.current_match = 0;
        self.last_pos = None;

        if self.query.is_empty() || buffer.text.is_empty() {
            return;
        }

        let mut search_from = 0;
        while let Some(pos) = buffer.text[search_from..].find(&self.query) {
            let abs_pos = search_from + pos;
            self.matches.push(FindMatch {
                start: abs_pos,
                end: abs_pos + self.query.len(),
            });
            search_from = abs_pos + self.query.len();
        }

        self.match_count = self.matches.len();
    }

    pub fn find_next(&mut self, buffer: &mut TextBuffer) {
        if self.query.is_empty() {
            self.match_count = 0;
            return;
        }

        let search_from = match self.last_pos {
            Some(pos) if pos + self.query.len() < buffer.text.len() => pos + self.query.len(),
            _ => 0,
        };

        let search = &buffer.text[search_from..];
        if let Some(pos) = search.find(&self.query) {
            let abs_pos = search_from + pos;
            self.last_pos = Some(abs_pos);
            self.match_count = buffer.text.matches(&self.query).count();
            buffer.set_cursor(abs_pos);
            self.current_match = self
                .matches
                .iter()
                .position(|m| m.start == abs_pos)
                .unwrap_or(0)
                + 1;
        } else if !buffer.text.is_empty() {
            let end = search_from.saturating_sub(1);
            let search = &buffer.text[..end];
            if let Some(pos) = search.find(&self.query) {
                self.last_pos = Some(pos);
                buffer.set_cursor(pos);
                self.current_match = self
                    .matches
                    .iter()
                    .position(|m| m.start == pos)
                    .unwrap_or(0)
                    + 1;
            } else {
                self.last_pos = None;
                self.match_count = 0;
                self.current_match = 0;
            }
        }
    }

    pub fn find_prev(&mut self, buffer: &mut TextBuffer) {
        if self.query.is_empty() {
            self.match_count = 0;
            return;
        }

        let end_bound = match self.last_pos {
            Some(pos) if pos >= self.query.len() => pos.saturating_sub(self.query.len()),
            _ => buffer.text.len(),
        };

        let search = &buffer.text[..end_bound];
        if let Some(pos) = search.rfind(&self.query) {
            self.last_pos = Some(pos);
            self.match_count = buffer.text.matches(&self.query).count();
            buffer.set_cursor(pos);
            self.current_match = self
                .matches
                .iter()
                .position(|m| m.start == pos)
                .unwrap_or(self.match_count.saturating_sub(1))
                + 1;
        } else {
            self.last_pos = None;
            self.match_count = 0;
            self.current_match = 0;
        }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.match_count = 0;
        self.current_match = 0;
        self.matches.clear();
        self.last_pos = None;
    }
}
