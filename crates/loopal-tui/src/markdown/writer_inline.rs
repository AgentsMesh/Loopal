/// Inline-level markdown event handling: emphasis, strong, strikethrough,
/// inline code, links, text, and breaks.
use ratatui::prelude::*;

use super::writer::MdWriter;

impl MdWriter {
    // ---- Emphasis (italic) ----

    pub(super) fn start_emphasis(&mut self) {
        self.push_style(self.styles.emphasis);
    }

    pub(super) fn end_emphasis(&mut self) {
        self.pop_style();
    }

    // ---- Strong (bold) ----

    pub(super) fn start_strong(&mut self) {
        self.push_style(self.styles.strong);
    }

    pub(super) fn end_strong(&mut self) {
        self.pop_style();
    }

    // ---- Strikethrough ----

    pub(super) fn start_strikethrough(&mut self) {
        self.push_style(self.styles.strikethrough);
    }

    pub(super) fn end_strikethrough(&mut self) {
        self.pop_style();
    }

    // ---- Links ----

    pub(super) fn start_link(&mut self, _dest: String) {
        self.push_style(self.styles.link);
    }

    pub(super) fn end_link(&mut self) {
        self.pop_style();
    }

    // ---- Text ----

    pub(super) fn on_text(&mut self, text: &str) {
        if self.in_code_block {
            // Accumulate into code buffer (don't add separators)
            self.code_buffer.push_str(text);
            return;
        }
        let style = self.current_style();
        self.pending_spans
            .push(Span::styled(text.to_string(), style));
    }

    // ---- Inline code ----

    pub(super) fn on_inline_code(&mut self, code: &str) {
        let style = self.current_style().patch(self.styles.code_inline);
        let text = format!("`{}`", code);
        self.pending_spans
            .push(Span::styled(text, style));
    }

    // ---- Breaks ----

    pub(super) fn on_soft_break(&mut self) {
        // Treat soft break as a space (standard markdown behavior)
        let style = self.current_style();
        self.pending_spans
            .push(Span::styled(" ".to_string(), style));
    }

    pub(super) fn on_hard_break(&mut self) {
        self.flush_pending();
    }
}
