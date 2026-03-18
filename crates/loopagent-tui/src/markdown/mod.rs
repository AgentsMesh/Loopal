/// Markdown rendering: pulldown-cmark parsing + syntect syntax highlighting.
///
/// Public API:
/// - `render_markdown(input, width)` → styled `Vec<Line<'static>>`
mod highlight;
mod styled_wrap;
mod styles;
mod writer;
mod writer_blocks;
mod writer_inline;

use ratatui::prelude::*;

use writer::MdWriter;

/// Parse markdown `input` and return pre-wrapped styled lines.
///
/// - Paragraphs and headings are word-wrapped to `width`.
/// - Code blocks are syntax-highlighted and **not** wrapped.
/// - Lists and blockquotes use indented prefixes.
pub fn render_markdown(input: &str, width: u16) -> Vec<Line<'static>> {
    if input.is_empty() {
        return Vec::new();
    }
    let writer = MdWriter::new(width);
    writer.render(input)
}
