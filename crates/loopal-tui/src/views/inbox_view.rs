/// Inbox view: shows pending messages (dynamic 0-3 lines).
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::inbox::Inbox;

/// Calculate the height needed for the inbox view (0-3 lines).
pub fn inbox_height(inbox: &Inbox) -> u16 {
    let count = inbox.len();
    if count == 0 {
        0
    } else {
        (count as u16).min(3)
    }
}

/// Render the inbox pending messages area.
pub fn render_inbox(f: &mut Frame, inbox: &Inbox, area: Rect) {
    if inbox.is_empty() || area.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (i, msg) in inbox.iter().enumerate() {
        if i >= 3 {
            break;
        }
        let truncated = truncate_at_char_boundary(msg, 57);
        let display = if truncated.len() < msg.len() {
            format!("  {truncated}...")
        } else {
            format!("  {msg}")
        };
        lines.push(Line::from(Span::styled(
            display,
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, area);
}

/// Truncate a string at a char boundary, returning at most `max_bytes` bytes.
fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
