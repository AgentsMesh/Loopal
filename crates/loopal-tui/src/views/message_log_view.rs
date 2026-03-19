/// Message feed view — global inter-agent communication display.
///
/// Shows recent `[source→target] preview` entries from the Observation Plane.
/// Collapses to 0 height when no messages have been routed.
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::message_log::MessageFeed;

/// Maximum visible rows for the message feed.
const MAX_FEED_LINES: u16 = 3;

/// Compute the height needed for the message feed area.
/// Returns 0 if empty, otherwise min(entry_count, MAX_FEED_LINES).
pub fn feed_height(feed: &MessageFeed) -> u16 {
    if feed.is_empty() {
        0
    } else {
        (feed.len() as u16).min(MAX_FEED_LINES)
    }
}

/// Render the most recent message feed entries.
///
/// Format per line: ` [source→target] preview_text`
pub fn render_message_feed(f: &mut Frame, feed: &MessageFeed, area: Rect) {
    if area.height == 0 || feed.is_empty() {
        return;
    }

    let visible = area.height as usize;
    let lines: Vec<Line<'static>> = feed
        .recent(visible)
        .map(|entry| {
            let header = Span::styled(
                format!(" [{}→{}] ", entry.source, entry.target),
                Style::default().fg(Color::DarkGray),
            );
            let preview = Span::styled(
                entry.content_preview.clone(),
                Style::default().fg(Color::Gray),
            );
            Line::from(vec![header, preview])
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, area);
}
