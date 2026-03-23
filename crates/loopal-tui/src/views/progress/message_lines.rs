/// Message lines conversion: DisplayMessage → Vec<Line<'static>>.
///
/// Instruction model: user messages show in a tinted background block
/// with a left accent bar for dark-mode readability. Assistant output
/// flows directly without labels, tool calls are single-line work traces,
/// thinking is a collapsed indicator.
use ratatui::prelude::*;
use unicode_width::UnicodeWidthStr;

use crate::markdown;
use loopal_session::types::DisplayMessage;

use super::tool_summary::tool_call_summary;

/// Convert a single DisplayMessage into pre-wrapped styled Lines.
pub fn message_to_lines(msg: &DisplayMessage, width: u16) -> Vec<Line<'static>> {
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

    // Tool calls — single-line summaries
    for tc in &msg.tool_calls {
        let (summary, color) = tool_call_summary(tc);
        let style = match color {
            "green" => Style::default().fg(Color::Green),
            "red" => Style::default().fg(Color::Red),
            _ => Style::default().fg(Color::Yellow),
        };
        let w = (width as usize).max(1);
        lines.extend(
            textwrap::wrap(&summary, w)
                .into_iter()
                .map(|cow| Line::from(Span::styled(cow.into_owned(), style))),
        );
    }

    lines.push(Line::from(""));
    lines
}

/// User message: tinted background block with left accent bar.
///
/// ```text
/// ▎ user message text padded to full width         ██████
/// ▎ continuation line                              ██████
/// ```
fn render_user(lines: &mut Vec<Line<'static>>, msg: &DisplayMessage, width: u16) {
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
fn render_assistant(lines: &mut Vec<Line<'static>>, msg: &DisplayMessage, width: u16) {
    if !msg.content.is_empty() {
        lines.extend(markdown::render_markdown(&msg.content, width));
    }
}

/// Thinking: collapsed to single-line indicator with token estimate.
fn render_thinking(lines: &mut Vec<Line<'static>>, msg: &DisplayMessage) {
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
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::DIM),
    )));
}

/// Welcome banner: ASCII art logo + slogan + model/path info.
///
/// Content format: "model\npath" (two lines separated by newline).
fn render_welcome(lines: &mut Vec<Line<'static>>, msg: &DisplayMessage) {
    let mut parts = msg.content.splitn(2, '\n');
    let model = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    // ASCII art "LOOPAL" — gradient from green to cyan
    // Each row gets a slightly different color to create a vertical gradient effect
    let logo_lines: &[&str] = &[
        r"  _        ___    ___   ____     _     _     ",
        r" | |      / _ \  / _ \ |  _ \   / \   | |    ",
        r" | |     | | | || | | || |_) | / _ \  | |    ",
        r" | |___  | |_| || |_| ||  __/ / ___ \ | |___ ",
        r" |_____|  \___/  \___/ |_|   /_/   \_\|_____|",
    ];

    let gradient: &[Color] = &[
        Color::Rgb(80, 200, 120), // green
        Color::Rgb(70, 200, 150), // green-teal
        Color::Rgb(60, 195, 180), // teal
        Color::Rgb(50, 190, 210), // teal-cyan
        Color::Rgb(40, 180, 230), // cyan
    ];

    lines.push(Line::from(""));
    for (i, logo_line) in logo_lines.iter().enumerate() {
        let color = gradient[i % gradient.len()];
        lines.push(Line::from(Span::styled(
            logo_line.to_string(),
            Style::default().fg(color).bold(),
        )));
    }

    // Slogan line
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  Rooted in code, Growing with ",
            Style::default().fg(Color::Rgb(140, 150, 170)),
        ),
        Span::styled(
            "loopal",
            Style::default().fg(Color::Rgb(60, 195, 180)).bold(),
        ),
        Span::styled(".", Style::default().fg(Color::Rgb(140, 150, 170))),
    ]));
    lines.push(Line::from(Span::styled(
        "  Part of AgentsMesh.ai",
        Style::default().fg(Color::Rgb(100, 110, 130)),
    )));

    // Info section: model + working directory
    lines.push(Line::from(""));
    if !model.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                "  model:     ",
                Style::default()
                    .fg(Color::Rgb(100, 110, 130))
                    .add_modifier(Modifier::DIM),
            ),
            Span::styled(model.to_string(), Style::default().fg(Color::Cyan)),
        ]));
    }
    if !path.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                "  directory: ",
                Style::default()
                    .fg(Color::Rgb(100, 110, 130))
                    .add_modifier(Modifier::DIM),
            ),
            Span::styled(
                path.to_string(),
                Style::default().fg(Color::Rgb(160, 170, 190)),
            ),
        ]));
    }
}

/// Generic prefixed message (error, system, unknown roles).
fn render_prefixed(
    lines: &mut Vec<Line<'static>>,
    msg: &DisplayMessage,
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
/// Streaming text is **incomplete** markdown — plain textwrap only.
/// No label prefix (instruction model: agent output IS the content).
pub fn streaming_to_lines(text: &str, width: u16) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for line in text.lines() {
        lines.extend(wrap_line(line, width));
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
