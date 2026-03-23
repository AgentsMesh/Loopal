/// Inline-level markdown event handling: emphasis, strong, strikethrough,
/// inline code, links, images, task list markers, footnote references,
/// text, and breaks.
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

    pub(super) fn start_link(&mut self, dest: String) {
        self.link_url = Some(dest);
        self.push_style(self.styles.link);
    }

    pub(super) fn end_link(&mut self) {
        self.pop_style();
        // Append URL in dim style after link text: text (url)
        if let Some(url) = self.link_url.take()
            && !url.is_empty()
        {
            let dim = Style::default().fg(Color::DarkGray);
            self.pending_spans
                .push(Span::styled(format!(" ({url})"), dim));
        }
    }

    // ---- Images ----

    pub(super) fn start_image(&mut self) {
        // Image text events will be collected as pending spans; we just
        // push an opening marker so end_image can wrap them.
        let dim = Style::default().fg(Color::DarkGray);
        self.pending_spans.push(Span::styled("[image: ", dim));
    }

    pub(super) fn end_image(&mut self) {
        let dim = Style::default().fg(Color::DarkGray);
        // If no alt text was collected, show generic placeholder
        let has_text = self.pending_spans.iter().any(|s| {
            let content = s.content.as_ref();
            content != "[image: " && !content.is_empty()
        });
        if !has_text
            && let Some(last) = self.pending_spans.last_mut()
            && last.content.as_ref() == "[image: "
        {
            *last = Span::styled("[image]", dim);
            return;
        }
        self.pending_spans.push(Span::styled("]", dim));
    }

    // ---- Task list marker ----

    pub(super) fn on_task_list_marker(&mut self, checked: bool) {
        let marker = if checked { "[x] " } else { "[ ] " };
        let style = if checked {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        self.pending_spans.push(Span::styled(marker, style));
    }

    // ---- Footnote reference ----

    pub(super) fn on_footnote_ref(&mut self, label: &str) {
        let dim = Style::default().fg(Color::DarkGray);
        self.pending_spans
            .push(Span::styled(format!("[^{label}]"), dim));
    }

    // ---- Text ----

    pub(super) fn on_text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_buffer.push_str(text);
            return;
        }
        if self.in_table {
            self.current_cell.push_str(text);
            return;
        }
        let style = self.current_style();
        self.pending_spans
            .push(Span::styled(text.to_string(), style));
    }

    // ---- Inline code ----

    pub(super) fn on_inline_code(&mut self, code: &str) {
        if self.in_table {
            self.current_cell.push_str(&format!("`{code}`"));
            return;
        }
        let style = self.current_style().patch(self.styles.code_inline);
        self.pending_spans
            .push(Span::styled(format!("`{code}`"), style));
    }

    // ---- Breaks ----

    pub(super) fn on_soft_break(&mut self) {
        let style = self.current_style();
        self.pending_spans
            .push(Span::styled(" ".to_string(), style));
    }

    pub(super) fn on_hard_break(&mut self) {
        self.flush_pending();
    }
}
