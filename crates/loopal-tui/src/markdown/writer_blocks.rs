use pulldown_cmark::{CodeBlockKind, Tag, TagEnd};
use ratatui::prelude::*;

use super::highlight::highlight_code_to_lines;
use super::styled_wrap::styled_wrap;
use super::writer::MdWriter;

impl MdWriter {
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
            Tag::Table(alignments) => self.start_table(alignments),
            Tag::TableHead => self.start_table_head(),
            Tag::TableRow => self.start_table_row(),
            Tag::TableCell => self.start_table_cell(),
            Tag::Emphasis => self.start_emphasis(),
            Tag::Strong => self.start_strong(),
            Tag::Strikethrough => self.start_strikethrough(),
            Tag::Link { dest_url, .. } => self.start_link(dest_url.to_string()),
            Tag::Image { .. } => self.start_image(),
            _ => {}
        }
    }

    pub(super) fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.end_paragraph(),
            TagEnd::Heading(_) => self.end_heading(),
            TagEnd::CodeBlock => self.end_code_block(),
            TagEnd::BlockQuote(_) => self.end_blockquote(),
            TagEnd::List(_) => self.end_list(),
            TagEnd::Item => self.end_item(),
            TagEnd::Table => self.end_table(),
            TagEnd::TableHead => self.end_table_head(),
            TagEnd::TableRow => self.end_table_row(),
            TagEnd::TableCell => self.end_table_cell(),
            TagEnd::Emphasis => self.end_emphasis(),
            TagEnd::Strong => self.end_strong(),
            TagEnd::Strikethrough => self.end_strikethrough(),
            TagEnd::Link => self.end_link(),
            TagEnd::Image => self.end_image(),
            _ => {}
        }
    }

    fn end_paragraph(&mut self) {
        self.flush_pending();
        self.lines.push(Line::from(""));
    }

    fn end_heading(&mut self) {
        let level = self.heading_level.take().unwrap_or(1);
        let heading_style = self.styles.heading(level);

        let spans: Vec<Span<'static>> = std::mem::take(&mut self.pending_spans)
            .into_iter()
            .map(|s| Span::styled(s.content.into_owned(), s.style.patch(heading_style)))
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
            .map(|ctx| ctx.prefix.iter().map(|s| s.content.len()).sum::<usize>() as u16)
            .sum()
    }

    fn start_code_block(&mut self, kind: CodeBlockKind) {
        self.flush_pending();
        self.in_code_block = true;
        self.code_buffer.clear();
        self.code_lang = match kind {
            CodeBlockKind::Fenced(lang) => {
                let l = lang.split(',').next().unwrap_or("").trim();
                if l.is_empty() {
                    None
                } else {
                    Some(l.to_string())
                }
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

    pub(super) fn on_rule(&mut self) {
        self.flush_pending();
        let w = self.width.saturating_sub(self.indent_width_calc()) as usize;
        let rule = "─".repeat(w.clamp(3, 80));
        self.lines
            .push(Line::from(Span::styled(rule, self.styles.rule)));
        self.lines.push(Line::from(""));
    }
}

fn heading_num(level: pulldown_cmark::HeadingLevel) -> u8 {
    use pulldown_cmark::HeadingLevel::*;
    match level {
        H1 => 1,
        H2 => 2,
        H3 => 3,
        H4 => 4,
        H5 => 5,
        H6 => 6,
    }
}
