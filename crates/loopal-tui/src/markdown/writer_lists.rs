use ratatui::prelude::*;

use super::writer::{IndentCtx, ListKind, MdWriter};

impl MdWriter {
    pub(super) fn start_list(&mut self, start: Option<u64>) {
        self.flush_pending();
        match start {
            Some(n) => self.list_stack.push(ListKind::Ordered(n)),
            None => self.list_stack.push(ListKind::Unordered),
        }
    }

    pub(super) fn end_list(&mut self) {
        self.flush_pending();
        self.list_stack.pop();
        if self.list_stack.is_empty() {
            self.lines.push(Line::from(""));
        }
    }

    pub(super) fn start_item(&mut self) {
        self.flush_pending();
        let marker_style = self.styles.list_marker;
        let (marker_text, indent_text) = match self.list_stack.last_mut() {
            Some(ListKind::Unordered) => ("- ".to_string(), "  ".to_string()),
            Some(ListKind::Ordered(n)) => {
                let m = format!("{n}. ");
                let indent = " ".repeat(m.len());
                *n += 1;
                (m, indent)
            }
            None => ("- ".to_string(), "  ".to_string()),
        };

        self.indent_stack.push(IndentCtx {
            prefix: vec![Span::raw(indent_text)],
            marker: Some(vec![Span::styled(marker_text, marker_style)]),
        });
    }

    pub(super) fn end_item(&mut self) {
        self.flush_pending();
        self.indent_stack.pop();
    }

    pub(super) fn start_blockquote(&mut self) {
        self.flush_pending();
        let style = self.styles.blockquote_marker;
        self.indent_stack.push(IndentCtx {
            prefix: vec![Span::styled("> ", style)],
            marker: None,
        });
    }

    pub(super) fn end_blockquote(&mut self) {
        self.flush_pending();
        self.indent_stack.pop();
    }
}
