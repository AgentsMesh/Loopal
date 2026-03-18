/// Block-level markdown event handling: paragraphs, headings, code blocks,
/// lists, blockquotes, and horizontal rules.
use pulldown_cmark::{CodeBlockKind, Tag, TagEnd};
use ratatui::prelude::*;

use super::highlight::highlight_code_to_lines;
use super::styled_wrap::styled_wrap;
use super::writer::{IndentCtx, ListKind, MdWriter};

impl MdWriter {
    /// Handle start of a block-level tag.
    pub(super) fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.heading_level = Some(heading_num(level));
            }
            Tag::CodeBlock(kind) => self.start_code_block(kind),
            Tag::BlockQuote(_) => self.start_blockquote(),
            Tag::List(start) => self.start_list(start),
            Tag::Item => self.start_item(),
            // Inline tags — delegated to writer_inline
            Tag::Emphasis => self.start_emphasis(),
            Tag::Strong => self.start_strong(),
            Tag::Strikethrough => self.start_strikethrough(),
            Tag::Link { dest_url, .. } => self.start_link(dest_url.to_string()),
            _ => {}
        }
    }

    /// Handle end of a block-level tag.
    pub(super) fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.end_paragraph(),
            TagEnd::Heading(_) => self.end_heading(),
            TagEnd::CodeBlock => self.end_code_block(),
            TagEnd::BlockQuote(_) => self.end_blockquote(),
            TagEnd::List(_) => self.end_list(),
            TagEnd::Item => self.end_item(),
            // Inline tags — delegated to writer_inline
            TagEnd::Emphasis => self.end_emphasis(),
            TagEnd::Strong => self.end_strong(),
            TagEnd::Strikethrough => self.end_strikethrough(),
            TagEnd::Link => self.end_link(),
            _ => {}
        }
    }

    // ---- Paragraphs ----

    fn end_paragraph(&mut self) {
        self.flush_pending();
        self.lines.push(Line::from(""));
    }

    // ---- Headings ----

    fn end_heading(&mut self) {
        let level = self.heading_level.take().unwrap_or(1);
        let heading_style = self.styles.heading(level);

        // Collect pending spans and apply heading style
        let spans: Vec<Span<'static>> = std::mem::take(&mut self.pending_spans)
            .into_iter()
            .map(|s| {
                Span::styled(s.content.into_owned(), s.style.patch(heading_style))
            })
            .collect();

        let line = Line::from(spans);
        let avail = self.width.saturating_sub(self.indent_width_calc()).max(1);
        let wrapped = styled_wrap(&line, avail);
        for wl in wrapped {
            self.lines.push(wl);
        }
        self.lines.push(Line::from(""));
    }

    fn indent_width_calc(&self) -> u16 {
        self.indent_stack
            .iter()
            .map(|ctx| {
                ctx.prefix.iter().map(|s| s.content.len()).sum::<usize>() as u16
            })
            .sum()
    }

    // ---- Code blocks ----

    fn start_code_block(&mut self, kind: CodeBlockKind) {
        self.flush_pending();
        self.in_code_block = true;
        self.code_buffer.clear();
        self.code_lang = match kind {
            CodeBlockKind::Fenced(lang) => {
                let l = lang.split(',').next().unwrap_or("").trim();
                if l.is_empty() { None } else { Some(l.to_string()) }
            }
            CodeBlockKind::Indented => None,
        };
    }

    fn end_code_block(&mut self) {
        self.in_code_block = false;
        let lang = self.code_lang.take().unwrap_or_default();
        let code = std::mem::take(&mut self.code_buffer);
        let highlighted = highlight_code_to_lines(&code, &lang);
        for line in highlighted {
            self.emit_raw(line);
        }
        self.lines.push(Line::from(""));
    }

    // ---- Lists ----

    fn start_list(&mut self, start: Option<u64>) {
        self.flush_pending();
        match start {
            Some(n) => self.list_stack.push(ListKind::Ordered(n)),
            None => self.list_stack.push(ListKind::Unordered),
        }
    }

    fn end_list(&mut self) {
        self.flush_pending();
        self.list_stack.pop();
        // Add blank line after top-level list
        if self.list_stack.is_empty() {
            self.lines.push(Line::from(""));
        }
    }

    fn start_item(&mut self) {
        self.flush_pending();
        let marker_style = self.styles.list_marker;
        let (marker_text, indent_text) = match self.list_stack.last_mut() {
            Some(ListKind::Unordered) => ("- ".to_string(), "  ".to_string()),
            Some(ListKind::Ordered(n)) => {
                let m = format!("{}. ", n);
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

    fn end_item(&mut self) {
        self.flush_pending();
        self.indent_stack.pop();
    }

    // ---- Blockquotes ----

    fn start_blockquote(&mut self) {
        self.flush_pending();
        let style = self.styles.blockquote_marker;
        self.indent_stack.push(IndentCtx {
            prefix: vec![Span::styled("> ", style)],
            marker: None,
        });
    }

    fn end_blockquote(&mut self) {
        self.flush_pending();
        self.indent_stack.pop();
    }

    // ---- Horizontal rule ----

    pub(super) fn on_rule(&mut self) {
        self.flush_pending();
        let w = self.width.saturating_sub(self.indent_width_calc()) as usize;
        let rule = "─".repeat(w.clamp(3, 80));
        self.lines.push(Line::from(Span::styled(
            rule,
            self.styles.rule,
        )));
        self.lines.push(Line::from(""));
    }
}

fn heading_num(level: pulldown_cmark::HeadingLevel) -> u8 {
    match level {
        pulldown_cmark::HeadingLevel::H1 => 1,
        pulldown_cmark::HeadingLevel::H2 => 2,
        pulldown_cmark::HeadingLevel::H3 => 3,
        pulldown_cmark::HeadingLevel::H4 => 4,
        pulldown_cmark::HeadingLevel::H5 => 5,
        pulldown_cmark::HeadingLevel::H6 => 6,
    }
}
