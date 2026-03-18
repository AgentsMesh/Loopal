/// Message lines conversion: DisplayMessage → Vec<Line<'static>>.
///
/// Content is rendered through the markdown pipeline (pulldown-cmark +
/// syntect highlighting). Tool calls remain plain textwrap.
use ratatui::prelude::*;

use crate::markdown;
use loopagent_session::types::{DisplayMessage, DisplayToolCall};

/// Convert a single DisplayMessage into pre-wrapped styled Lines.
pub fn message_to_lines(msg: &DisplayMessage, width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Role label (always short — no wrap needed)
    let (label, style) = match msg.role.as_str() {
        "user" => ("You", Style::default().fg(Color::Green).bold()),
        "assistant" => ("Agent", Style::default().fg(Color::Cyan).bold()),
        "error" => ("Error", Style::default().fg(Color::Red).bold()),
        "system" => ("System", Style::default().fg(Color::Yellow).bold()),
        other => (other, Style::default().bold()),
    };
    lines.push(Line::from(Span::styled(format!("{}: ", label), style)));

    // Content — markdown rendering for assistant, plain wrap for others
    if !msg.content.is_empty() {
        if msg.role == "assistant" {
            lines.extend(markdown::render_markdown(&msg.content, width));
        } else {
            for line in msg.content.lines() {
                lines.extend(wrap_line(line, width));
            }
        }
    }

    // Tool calls — plain textwrap
    for tc in &msg.tool_calls {
        lines.extend(tool_call_lines(tc, width));
    }

    lines.push(Line::from(""));
    lines
}

/// Convert a DisplayToolCall into pre-wrapped styled Lines.
///
/// Two-phase rendering:
/// 1. Header line — status icon + summary (call description)
/// 2. Result content — truncated to MAX_RESULT_DISPLAY_LINES for display
const MAX_RESULT_DISPLAY_LINES: usize = 10;

fn tool_call_lines(tc: &DisplayToolCall, width: u16) -> Vec<Line<'static>> {
    let icon = match tc.status.as_str() {
        "success" => "+",
        "error" => "x",
        _ => "~",
    };
    let header_style = match tc.status.as_str() {
        "success" => Style::default().fg(Color::Green),
        "error" => Style::default().fg(Color::Red),
        _ => Style::default().fg(Color::Yellow),
    };

    // Phase 1: header — "[icon] summary"
    let header = format!("  [{}] {}", icon, tc.summary);
    let w = (width as usize).max(1);
    let mut lines: Vec<Line<'static>> = textwrap::wrap(&header, w)
        .into_iter()
        .map(|cow| Line::from(Span::styled(cow.into_owned(), header_style)))
        .collect();

    // Phase 2: result content (display-time truncation)
    if let Some(ref result) = tc.result
        && !result.is_empty()
    {
        let result_style = if tc.status == "error" {
            Style::default().fg(Color::Red).add_modifier(Modifier::DIM)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        let all: Vec<&str> = result.lines().collect();
        let show = all.len().min(MAX_RESULT_DISPLAY_LINES);
        for line in &all[..show] {
            let indented = format!("      {}", line);
            lines.extend(
                textwrap::wrap(&indented, w)
                    .into_iter()
                    .map(|cow| Line::from(Span::styled(cow.into_owned(), result_style))),
            );
        }
        if all.len() > show {
            lines.push(Line::from(Span::styled(
                format!("      ... ({} more lines)", all.len() - show),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
    lines
}

/// Convert streaming text into pre-wrapped styled Lines.
///
/// Streaming text is **incomplete** markdown (unclosed code fences, inline
/// styles, etc.), so we must NOT run it through pulldown-cmark — the parser
/// produces unstable output that causes line-count fluctuation and display
/// corruption. Plain textwrap is used here; full markdown rendering is
/// applied once the message is committed to the message list.
pub fn streaming_to_lines(text: &str, width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if !text.is_empty() {
        lines.push(Line::from(Span::styled(
            "Agent: ".to_string(),
            Style::default().fg(Color::Cyan).bold(),
        )));
        for line in text.lines() {
            lines.extend(wrap_line(line, width));
        }
    }
    lines
}

/// Wrap a single logical line into visual lines using textwrap.
fn wrap_line(line: &str, width: u16) -> Vec<Line<'static>> {
    let w = (width as usize).max(1);
    textwrap::wrap(line, w)
        .into_iter()
        .map(|cow| Line::from(cow.into_owned()))
        .collect()
}
