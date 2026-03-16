use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

/// Render the chat message list into the given area.
pub fn render_chat(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Chat ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Clear the inner area to prevent rendering artifacts from previous frames.
    // This is needed because Paragraph with Wrap and scroll can leave stale
    // characters when content changes or the user scrolls (especially with
    // CJK/wide characters that span multiple terminal cells).
    let clear = Paragraph::new("");
    f.render_widget(clear, inner);

    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        // Role label
        let (label, style) = match msg.role.as_str() {
            "user" => ("You", Style::default().fg(Color::Green).bold()),
            "assistant" => ("Agent", Style::default().fg(Color::Cyan).bold()),
            "error" => ("Error", Style::default().fg(Color::Red).bold()),
            "system" => ("System", Style::default().fg(Color::Yellow).bold()),
            other => (other, Style::default().bold()),
        };

        lines.push(Line::from(Span::styled(
            format!("{}: ", label),
            style,
        )));

        // Content lines
        if !msg.content.is_empty() {
            for line in msg.content.lines() {
                lines.push(Line::from(line.to_string()));
            }
        }

        // Tool calls
        for tc in &msg.tool_calls {
            let icon = match tc.status.as_str() {
                "success" => "+",
                "error" => "x",
                _ => "~",
            };
            let tc_style = match tc.status.as_str() {
                "success" => Style::default().fg(Color::Green),
                "error" => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::Yellow),
            };
            lines.push(Line::from(Span::styled(
                format!("  [{}] {}", icon, tc.summary),
                tc_style,
            )));
        }

        lines.push(Line::from(""));
    }

    // Inbox pending messages (queued but not yet forwarded to agent)
    for pending_msg in &app.inbox {
        lines.push(Line::from(Span::styled(
            "You (pending): ",
            Style::default().fg(Color::DarkGray).bold(),
        )));
        for line in pending_msg.lines() {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::DarkGray),
            )));
        }
        lines.push(Line::from(""));
    }

    // Streaming text (currently being received)
    if !app.streaming_text.is_empty() {
        lines.push(Line::from(Span::styled(
            "Agent: ",
            Style::default().fg(Color::Cyan).bold(),
        )));
        for line in app.streaming_text.lines() {
            lines.push(Line::from(line.to_string()));
        }
    }

    // Calculate scroll: show the bottom of the content
    let total_lines = lines.len() as u16;
    let visible = inner.height;
    let scroll = if app.scroll_offset > 0 {
        total_lines.saturating_sub(visible).saturating_sub(app.scroll_offset)
    } else {
        total_lines.saturating_sub(visible)
    };

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(paragraph, inner);
}
