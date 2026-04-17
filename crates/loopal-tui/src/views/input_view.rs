//! Input view: multiline `> ` prompt with auto-wrap and dynamic height.
//!
//! Supports hard newlines (Shift+Enter) and soft wrapping at terminal width.
//! Large paste placeholders are rendered in a distinct style.
//! Exports `input_height()` for the layout engine to compute dynamic height.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::text_width::display_width;

use crate::input::multiline;
use crate::input::paste;

/// Maximum visible height for the input area (rows).
pub const INPUT_MAX_HEIGHT: u16 = 8;

/// Render the input area as a multiline `> ` prompt with auto-wrap.
pub fn render_input(
    f: &mut Frame,
    input: &str,
    cursor: usize,
    image_count: usize,
    input_scroll: usize,
    area: Rect,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let prefix = build_prefix(image_count);
    let prefix_width = display_width(&prefix);
    let content_width = (area.width as usize).saturating_sub(prefix_width);

    // Build styled lines with prefix on the first line
    let lines = build_styled_lines(input, &prefix, content_width);

    // Apply vertical scroll
    let visible_lines: Vec<Line> = lines
        .into_iter()
        .skip(input_scroll)
        .take(area.height as usize)
        .collect();

    let para = Paragraph::new(visible_lines);
    f.render_widget(para, area);

    // Cursor position accounting for wrap and scroll
    set_cursor(
        f,
        input,
        cursor,
        prefix_width,
        content_width,
        input_scroll,
        area,
    );
}

/// Calculate how many rows the input needs (for layout).
pub fn input_height(input: &str, area_width: u16, prefix_width: usize) -> u16 {
    if input.is_empty() {
        return 1;
    }
    let content_width = (area_width as usize).saturating_sub(prefix_width);
    if content_width == 0 {
        return 1;
    }
    let lines = multiline::visual_lines(input, content_width);
    (lines.len() as u16).clamp(1, INPUT_MAX_HEIGHT)
}

/// Calculate the prefix display width (exported for layout computation).
pub fn prefix_width(image_count: usize) -> usize {
    display_width(&build_prefix(image_count))
}

// --- Internal helpers ---

/// Build styled lines: first visual line gets the prefix, continuation lines
/// get padding of the same width. Paste placeholders use a distinct style.
fn build_styled_lines<'a>(input: &'a str, prefix: &str, content_width: usize) -> Vec<Line<'a>> {
    let wrap_width = if content_width == 0 {
        usize::MAX
    } else {
        content_width
    };
    let vlines = multiline::visual_lines(input, wrap_width);
    let prefix_pad = " ".repeat(display_width(prefix));
    let prefix_style = Style::default().fg(Color::DarkGray);
    let placeholder_style = Style::default().fg(Color::DarkGray).italic();

    vlines
        .iter()
        .enumerate()
        .map(|(i, vl)| {
            let leading = if i == 0 {
                Span::styled(prefix.to_string(), prefix_style)
            } else {
                Span::styled(prefix_pad.clone(), prefix_style)
            };
            let slice = &input[vl.byte_start..vl.byte_start + vl.byte_len];
            let content_span = if paste::is_paste_placeholder(slice) {
                Span::styled(slice, placeholder_style)
            } else {
                Span::raw(slice)
            };
            Line::from(vec![leading, content_span])
        })
        .collect()
}

/// Set the terminal cursor at the correct (x, y) for the byte cursor.
fn set_cursor(
    f: &mut Frame,
    input: &str,
    cursor: usize,
    prefix_width: usize,
    content_width: usize,
    input_scroll: usize,
    area: Rect,
) {
    let wrap_width = if content_width == 0 {
        usize::MAX
    } else {
        content_width
    };
    let vlines = multiline::visual_lines(input, wrap_width);
    let (row, col) = multiline::cursor_to_row_col(input, cursor, &vlines);

    let visible_row = row.saturating_sub(input_scroll);
    if visible_row >= area.height as usize {
        return; // Cursor is outside the visible area
    }

    let x = area.x + (prefix_width + col) as u16;
    let y = area.y + visible_row as u16;
    f.set_cursor_position((x, y));
}

fn build_prefix(image_count: usize) -> String {
    if image_count > 0 {
        format!("> [img:{image_count}] ")
    } else {
        "> ".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_line_height() {
        assert_eq!(input_height("hello", 80, 2), 1);
    }

    #[test]
    fn test_multiline_height() {
        assert_eq!(input_height("a\nb\nc", 80, 2), 3);
    }

    #[test]
    fn test_wrap_height() {
        // 10 chars in a 5-col content area → 2 visual lines
        assert_eq!(input_height("abcdefghij", 7, 2), 2);
    }

    #[test]
    fn test_max_height_capped() {
        let text = "a\n".repeat(20);
        assert_eq!(input_height(&text, 80, 2), INPUT_MAX_HEIGHT);
    }

    #[test]
    fn test_empty_input_height() {
        assert_eq!(input_height("", 80, 2), 1);
    }

    #[test]
    fn test_prefix_variants() {
        assert_eq!(build_prefix(0), "> ");
        assert_eq!(build_prefix(1), "> [img:1] ");
        assert_eq!(build_prefix(2), "> [img:2] ");
    }

    #[test]
    fn test_prefix_width_fn() {
        assert_eq!(prefix_width(0), 2);
    }
}
