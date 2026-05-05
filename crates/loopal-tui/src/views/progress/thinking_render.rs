/// Thinking content rendering: full thinking text with dimmed gray-purple styling.
///
/// Display format matches the reference design:
/// - Header line: `· Thinking…` (italic) with optional token count
/// - Body: full thinking text, 4-space indented, dimmed gray-lavender
use ratatui::prelude::*;
use unicode_width::UnicodeWidthStr;

use loopal_view_state::{format_token_display, parse_thinking_content};

/// Dimmed gray-lavender for thinking body text.
const THINKING_TEXT: Color = Color::Rgb(155, 150, 170);
/// Slightly brighter for the header label.
const THINKING_HEADER: Color = Color::Rgb(180, 175, 195);
/// Body indentation (4 spaces).
const INDENT: &str = "    ";
const INDENT_W: usize = 4;

/// Render a completed thinking message (stored as `"{token_count}\n{text}"`).
pub fn render_thinking(lines: &mut Vec<Line<'static>>, content: &str, width: u16) {
    let (token_count, text) = parse_thinking_content(content);

    // Header: `· Thinking… (Xk tokens)`
    let header = if token_count > 0 {
        let display = format_token_display(token_count);
        format!("· Thinking… ({display} tokens)")
    } else {
        "· Thinking…".to_string()
    };
    let header_style = Style::default()
        .fg(THINKING_HEADER)
        .add_modifier(Modifier::ITALIC);
    lines.push(Line::from(Span::styled(header, header_style)));

    // Body: indented, wrapped thinking text
    if !text.is_empty() {
        lines.push(Line::from(""));
        append_indented_text(lines, text, width);
    }
}

/// Render streaming thinking content (during active thinking).
pub fn streaming_thinking_lines(text: &str, width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Header with estimated token count
    let token_est = text.len() as u32 / 4;
    let header = if token_est > 0 {
        let display = format_token_display(token_est);
        format!("· Thinking… ({display} tokens)")
    } else {
        "· Thinking…".to_string()
    };
    let header_style = Style::default()
        .fg(THINKING_HEADER)
        .add_modifier(Modifier::ITALIC);
    lines.push(Line::from(Span::styled(header, header_style)));

    // Show thinking body
    if !text.is_empty() {
        lines.push(Line::from(""));
        append_indented_text(&mut lines, text, width);
    }

    lines
}

/// Append text lines with 4-space indent and thinking body style.
fn append_indented_text(lines: &mut Vec<Line<'static>>, text: &str, width: u16) {
    let body_style = Style::default().fg(THINKING_TEXT);
    let indent_style = body_style;
    let wrap_w = (width as usize).saturating_sub(INDENT_W).max(1);

    for line in text.lines() {
        if line.is_empty() {
            lines.push(Line::from(""));
            continue;
        }
        // Fast path: no wrapping needed
        if UnicodeWidthStr::width(line) <= wrap_w {
            lines.push(Line::from(vec![
                Span::styled(INDENT.to_string(), indent_style),
                Span::styled(line.to_string(), body_style),
            ]));
        } else {
            for cow in textwrap::wrap(line, wrap_w) {
                lines.push(Line::from(vec![
                    Span::styled(INDENT.to_string(), indent_style),
                    Span::styled(cow.into_owned(), body_style),
                ]));
            }
        }
    }
}
