use super::buffer::TextBuffer;

pub struct FindState {
    pub query: String,
    pub match_count: usize,
    last_pos: Option<usize>,
}

impl Default for FindState {
    fn default() -> Self {
        Self {
            query: String::new(),
            match_count: 0,
            last_pos: None,
        }
    }
}

impl FindState {
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
        } else if !buffer.text.is_empty() {
            let end = search_from.saturating_sub(1);
            let search = &buffer.text[..end];
            if let Some(pos) = search.find(&self.query) {
                self.last_pos = Some(pos);
                buffer.set_cursor(pos);
            } else {
                self.last_pos = None;
                self.match_count = 0;
            }
        }
    }

    pub fn reset(&mut self) {
        self.match_count = 0;
        self.last_pos = None;
    }
}
