use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuestionState {
    #[serde(default)]
    pub selection: Vec<bool>,
    #[serde(default)]
    pub other_selected: bool,
    #[serde(default)]
    pub free_text: String,
    #[serde(default)]
    pub free_text_cursor: usize,
    #[serde(default)]
    pub cursor: usize,
    #[serde(default)]
    pub interacted: bool,
}

impl QuestionState {
    pub(crate) fn new(option_count: usize) -> Self {
        Self {
            selection: vec![false; option_count],
            ..Default::default()
        }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn other_selected(&self) -> bool {
        self.other_selected
    }
    pub fn free_text(&self) -> &str {
        &self.free_text
    }
    pub fn free_text_cursor(&self) -> usize {
        self.free_text_cursor
    }
    pub fn selection(&self) -> &[bool] {
        &self.selection
    }

    pub fn interacted(&self) -> bool {
        self.interacted
    }

    /// Move cursor (clamped) and mark this state as user-interacted.
    /// Use this for user-driven cursor movement; programmatic cursor reset
    /// (deserialization, advance-to-next) should set the field directly.
    pub fn user_set_cursor_clamped(&mut self, c: usize, max: usize) {
        self.cursor = c.min(max);
        self.interacted = true;
    }

    pub(crate) fn insert_char(&mut self, c: char) {
        let cursor = self.free_text_cursor.min(self.free_text.chars().count());
        let byte = char_index_to_byte(&self.free_text, cursor);
        self.free_text.insert(byte, c);
        self.free_text_cursor = cursor + 1;
        self.interacted = true;
    }

    pub(crate) fn backspace(&mut self) {
        if self.free_text_cursor == 0 {
            return;
        }
        let cursor = self.free_text_cursor.min(self.free_text.chars().count());
        let prev = cursor - 1;
        let from = char_index_to_byte(&self.free_text, prev);
        let to = char_index_to_byte(&self.free_text, cursor);
        self.free_text.replace_range(from..to, "");
        self.free_text_cursor = prev;
        self.interacted = true;
    }

    pub(crate) fn delete(&mut self) {
        let len = self.free_text.chars().count();
        if self.free_text_cursor >= len {
            return;
        }
        let from = char_index_to_byte(&self.free_text, self.free_text_cursor);
        let to = char_index_to_byte(&self.free_text, self.free_text_cursor + 1);
        self.free_text.replace_range(from..to, "");
        self.interacted = true;
    }

    pub(crate) fn toggle_selection(&mut self) -> bool {
        if let Some(slot) = self.selection.get_mut(self.cursor) {
            *slot = !*slot;
            self.interacted = true;
            true
        } else {
            false
        }
    }

    pub(crate) fn toggle_other(&mut self) {
        self.other_selected = !self.other_selected;
        self.interacted = true;
    }

    pub(crate) fn cursor_left(&mut self) {
        if self.free_text_cursor > 0 {
            self.free_text_cursor -= 1;
        }
    }

    pub(crate) fn cursor_right(&mut self) {
        let len = self.free_text.chars().count();
        if self.free_text_cursor < len {
            self.free_text_cursor += 1;
        }
    }

    pub(crate) fn cursor_home(&mut self) {
        self.free_text_cursor = 0;
    }

    pub(crate) fn cursor_end(&mut self) {
        self.free_text_cursor = self.free_text.chars().count();
    }
}

fn char_index_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}
