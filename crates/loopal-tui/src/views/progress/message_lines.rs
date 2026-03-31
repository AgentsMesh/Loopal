/// Message lines conversion: SessionMessage → Vec<Line<'static>>.
///
/// Instruction model: user messages show in a tinted background block
/// with a left accent bar for dark-mode readability. Assistant output
/// flows directly without labels, tool calls are single-line work traces,
/// thinking is a collapsed indicator.
use ratatui::prelude::*;
use unicode_width::UnicodeWidthStr;

use crate::markdown;
use loopal_session::types::SessionMessage;

use super::skill_display::render_skill_invoke;
use super::tool_display::render_tool_calls;
use super::welcome::render_welcome;

/// Convert a single SessionMessage into pre-wrapped styled Lines.
pub fn message_to_lines(msg: &SessionMessage, width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    match msg.role.as_str() {
        "user" => render_user(&mut lines, msg, width),
        "assistant" => render_assistant(&mut lines, msg, width),
        "thinking" => render_thinking(&mut lines, msg),
        "welcome" => render_welcome(&mut lines, msg),
        "error" => render_prefixed(&mut lines, msg, "Error: ", Color::Red, width),
        "system" => render_prefixed(&mut lines, msg, "System: ", Color::Yellow, width),
        _ => render_prefixed(
            &mut lines,
            msg,
            &format!("{}: ", msg.role),
            Color::White,
            width,
        ),
    }

    // Tool calls — group-aware rendering
    if !msg.tool_calls.is_empty() {
        lines.extend(render_tool_calls(&msg.tool_calls, width));
    }

    lines.push(Line::from(""));
    lines
}

/// User message: tinted background block with left accent bar.
/// Skill invocations render as a collapsed summary instead of the full body.
fn render_user(lines: &mut Vec<Line<'static>>, msg: &SessionMessage, width: u16) {
    if let Some(skill) = &msg.skill_info {
        render_skill_invoke(lines, skill, width);
        return;
    }

    let w = (width as usize).max(1);
    let accent = Style::default()
        .fg(Color::Rgb(100, 130, 200))
        .bg(Color::Rgb(30, 35, 48));
    let text_style = Style::default()
        .fg(Color::Rgb(185, 190, 205))
        .bg(Color::Rgb(30, 35, 48));

    if msg.content.is_empty() {
        lines.push(user_line("", w, accent, text_style));
        return;
    }
    // Wrap at (width - 3) to reserve space for "▎ " prefix (3 cols)
    let inner_w = w.saturating_sub(3).max(1);
    for line in msg.content.lines() {
        for cow in textwrap::wrap(line, inner_w) {
            lines.push(user_line(&cow, w, accent, text_style));
        }
    }
}

/// Build a single user-message line: `▎ text<padding>`.
///
/// Pads with spaces to fill `total_width` so the background covers the row.
fn user_line(text: &str, total_width: usize, accent: Style, text_style: Style) -> Line<'static> {
    let prefix = "▎ ";
    let prefix_w = 3; // ▎(2) + space(1)
    let text_w = UnicodeWidthStr::width(text);
    let pad = total_width.saturating_sub(prefix_w + text_w);
    Line::from(vec![
        Span::styled(prefix.to_string(), accent),
        Span::styled(text.to_string(), text_style),
        Span::styled(" ".repeat(pad), text_style),
    ])
}

/// Assistant message: direct output, no label. Markdown rendered.
fn render_assistant(lines: &mut Vec<Line<'static>>, msg: &SessionMessage, width: u16) {
    if !msg.content.is_empty() {
        lines.extend(markdown::render_markdown(&msg.content, width));
    }
}

/// Thinking: collapsed to single-line indicator with token estimate.
fn render_thinking(lines: &mut Vec<Line<'static>>, msg: &SessionMessage) {
    let token_est = msg.content.len() / 4;
    let label = if token_est >= 1000 {
        format!("Thinking ({}k tokens)", token_est / 1000)
    } else if token_est > 0 {
        format!("Thinking ({token_est} tokens)")
    } else {
        "Thinking...".to_string()
    };
    lines.push(Line::from(Span::styled(
        label,
        Style::default().fg(Color::Rgb(180, 130, 210)),
    )));
}

/// Generic prefixed message (error, system, unknown roles).
fn render_prefixed(
    lines: &mut Vec<Line<'static>>,
    msg: &SessionMessage,
    prefix: &str,
    color: Color,
    width: u16,
) {
    let style = Style::default().fg(color).bold();
    lines.push(Line::from(Span::styled(prefix.to_string(), style)));
    if !msg.content.is_empty() {
        for line in msg.content.lines() {
            lines.extend(wrap_line(line, width));
        }
    }
}

/// Convert streaming text into pre-wrapped styled Lines.
///
/// Uses full markdown rendering so users see formatted output incrementally,
/// not raw markup that only resolves after the message completes.
/// pulldown-cmark handles incomplete markdown gracefully (e.g. unclosed
/// code fences render trailing content as code — correct during streaming).
pub fn streaming_to_lines(text: &str, width: u16) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }
    markdown::render_markdown(text, width)
}

/// Wrap a single logical line into visual lines using textwrap.
fn wrap_line(line: &str, width: u16) -> Vec<Line<'static>> {
    let w = (width as usize).max(1);
    textwrap::wrap(line, w)
        .into_iter()
        .map(|cow| Line::from(cow.into_owned()))
        .collect()
}
