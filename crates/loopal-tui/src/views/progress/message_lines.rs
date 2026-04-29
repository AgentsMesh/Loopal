use ratatui::prelude::*;
use unicode_width::UnicodeWidthStr;

use crate::markdown;
use loopal_protocol::MessageSource;
use loopal_session::types::{InboxOrigin, SessionMessage};

use super::skill_display::render_skill_invoke;
use super::thinking_render;
use super::tool_display::render_tool_calls;
use super::welcome::render_welcome;

pub fn message_to_lines(msg: &SessionMessage, width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    match msg.role.as_str() {
        "user" => render_user(&mut lines, msg, width),
        "assistant" => render_assistant(&mut lines, msg, width),
        "thinking" => thinking_render::render_thinking(&mut lines, &msg.content, width),
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

    if !msg.tool_calls.is_empty() {
        lines.extend(render_tool_calls(&msg.tool_calls, width));
    }

    lines.push(Line::from(""));
    lines
}

fn render_user(lines: &mut Vec<Line<'static>>, msg: &SessionMessage, width: u16) {
    if let Some(skill) = &msg.skill_info {
        render_skill_invoke(lines, skill, width);
        return;
    }
    if let Some(origin) = &msg.inbox
        && !matches!(origin.source, MessageSource::Human)
    {
        render_inbox_origin(lines, origin, width);
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
    let inner_w = w.saturating_sub(3).max(1);
    for line in msg.content.lines() {
        for cow in textwrap::wrap(line, inner_w) {
            lines.push(user_line(&cow, w, accent, text_style));
        }
    }
}

fn render_inbox_origin(lines: &mut Vec<Line<'static>>, origin: &InboxOrigin, width: u16) {
    let label = match &origin.source {
        MessageSource::Agent(addr) => format!("📨 from {addr}"),
        MessageSource::Scheduled => "⏰ scheduled".to_string(),
        MessageSource::Channel { channel, from } => format!("📡 #{channel}/{from}"),
        MessageSource::System(kind) => format!("⚙ system:{kind}"),
        MessageSource::Human => return,
    };
    let style = Style::default().fg(Color::Rgb(140, 170, 220)).italic();
    lines.push(Line::from(Span::styled(label, style)));
    if let Some(summary) = &origin.summary {
        let dim = Style::default().fg(Color::Rgb(120, 125, 140));
        for cow in textwrap::wrap(summary, (width as usize).max(1)) {
            lines.push(Line::from(Span::styled(cow.into_owned(), dim)));
        }
    }
}

fn user_line(text: &str, total_width: usize, accent: Style, text_style: Style) -> Line<'static> {
    let prefix = "▎ ";
    let prefix_w = 3;
    let text_w = UnicodeWidthStr::width(text);
    let pad = total_width.saturating_sub(prefix_w + text_w);
    Line::from(vec![
        Span::styled(prefix.to_string(), accent),
        Span::styled(text.to_string(), text_style),
        Span::styled(" ".repeat(pad), text_style),
    ])
}

fn render_assistant(lines: &mut Vec<Line<'static>>, msg: &SessionMessage, width: u16) {
    if !msg.content.is_empty() {
        lines.extend(markdown::render_markdown(&msg.content, width));
    }
}

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

pub fn streaming_to_lines(text: &str, width: u16) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }
    markdown::render_markdown(text, width)
}

fn wrap_line(line: &str, width: u16) -> Vec<Line<'static>> {
    let w = (width as usize).max(1);
    textwrap::wrap(line, w)
        .into_iter()
        .map(|cow| Line::from(cow.into_owned()))
        .collect()
}
