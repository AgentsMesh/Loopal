/// Markdown writer — pulldown-cmark event state machine.
///
/// Drives the conversion from markdown events to styled `Line`s.
/// Block-level and inline-level handling are split into sibling modules.
use pulldown_cmark::{Event, Options, Parser};
use ratatui::prelude::*;

use super::styles::MarkdownStyles;

/// Context for nested indentation (blockquote / list).
#[derive(Clone)]
pub(super) struct IndentCtx {
    /// Prefix prepended to every line inside this context (e.g. "> ").
    pub prefix: Vec<Span<'static>>,
    /// One-shot marker for the first line of a list item (e.g. "- ").
    pub marker: Option<Vec<Span<'static>>>,
}

/// Ordered vs unordered list tracking.
#[derive(Clone)]
pub(super) enum ListKind {
    Unordered,
    Ordered(u64),
}

/// Markdown → styled Lines converter.
pub(super) struct MdWriter {
    pub lines: Vec<Line<'static>>,
    pub styles: MarkdownStyles,
    pub style_stack: Vec<Style>,
    pub in_code_block: bool,
    pub code_lang: Option<String>,
    pub code_buffer: String,
    pub list_stack: Vec<ListKind>,
    pub indent_stack: Vec<IndentCtx>,
    pub pending_spans: Vec<Span<'static>>,
    pub width: u16,
    pub heading_level: Option<u8>,
}

impl MdWriter {
    pub fn new(width: u16) -> Self {
        Self {
            lines: Vec::new(),
            styles: MarkdownStyles::default(),
            style_stack: vec![Style::default()],
            in_code_block: false,
            code_lang: None,
            code_buffer: String::new(),
            list_stack: Vec::new(),
            indent_stack: Vec::new(),
            pending_spans: Vec::new(),
            width,
            heading_level: None,
        }
    }

    /// Run the full markdown-to-lines conversion.
    pub fn render(mut self, input: &str) -> Vec<Line<'static>> {
        let opts = Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES;
        let parser = Parser::new_ext(input, opts);
        for event in parser {
            self.handle_event(event);
        }
        self.flush_pending();
        self.lines
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.on_text(&text),
            Event::Code(code) => self.on_inline_code(&code),
            Event::SoftBreak => self.on_soft_break(),
            Event::HardBreak => self.on_hard_break(),
            Event::Rule => self.on_rule(),
            Event::Html(html) => self.on_text(&html),
            Event::InlineHtml(html) => self.on_text(&html),
            _ => {}
        }
    }

    /// Current active inline style (top of stack).
    pub fn current_style(&self) -> Style {
        self.style_stack.last().copied().unwrap_or_default()
    }

    /// Push a new style merged with the current style.
    pub fn push_style(&mut self, style: Style) {
        let merged = self.current_style().patch(style);
        self.style_stack.push(merged);
    }

    /// Pop the top style.
    pub fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    /// Flush pending spans as a single logical line.
    pub fn flush_pending(&mut self) {
        if self.pending_spans.is_empty() {
            return;
        }
        let spans: Vec<Span<'static>> =
            std::mem::take(&mut self.pending_spans);
        let line = Line::from(spans);
        self.emit_wrapped(line);
    }

    /// Wrap a line to width and append to output, respecting indent.
    pub fn emit_wrapped(&mut self, line: Line<'static>) {
        use super::styled_wrap::styled_wrap;

        let indent_w: u16 = self.indent_width();
        let avail = self.width.saturating_sub(indent_w).max(1);
        let wrapped = styled_wrap(&line, avail);

        for (i, mut wl) in wrapped.into_iter().enumerate() {
            let prefix = self.build_prefix(i == 0);
            if !prefix.is_empty() {
                let mut full = prefix;
                full.extend(wl.spans);
                wl = Line::from(full);
            }
            self.lines.push(wl);
        }
    }

    /// Emit a line without wrapping (used for code blocks).
    pub fn emit_raw(&mut self, line: Line<'static>) {
        let prefix = self.build_prefix(false);
        if prefix.is_empty() {
            self.lines.push(line);
        } else {
            let mut full = prefix;
            full.extend(line.spans);
            self.lines.push(Line::from(full));
        }
    }

    /// Calculate indent width from stack.
    fn indent_width(&self) -> u16 {
        self.indent_stack
            .iter()
            .map(|ctx| {
                let pw: usize = ctx
                    .prefix
                    .iter()
                    .map(|s| s.content.len())
                    .sum();
                pw as u16
            })
            .sum()
    }

    /// Build prefix spans for current line.
    /// `first_in_item`: true if this is the first line of a list item.
    fn build_prefix(&mut self, first_in_item: bool) -> Vec<Span<'static>> {
        let mut prefix = Vec::new();
        for ctx in &mut self.indent_stack {
            if first_in_item
                && let Some(marker) = ctx.marker.take()
            {
                prefix.extend(marker);
                continue;
            }
            prefix.extend(ctx.prefix.iter().cloned());
        }
        prefix
    }
}
