//! Transient retry error banner — in-place overlay between separator and input.
//!
//! Appears during LLM API retries, auto-clears on success.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Render the retry error banner (1 row, yellow text on dark background).
///
/// ```text
/// ⟳ API error: status=502. Retrying in 4.0s (2/6)
/// ```
pub fn render_retry_banner(f: &mut Frame, message: &str, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default()
        .fg(Color::Yellow)
        .bg(Color::Rgb(50, 35, 15));
    let line = Line::from(vec![
        Span::styled("⟳ ", style.bold()),
        Span::styled(message.to_string(), style),
    ]);
    f.render_widget(Paragraph::new(line).style(style), area);
}

/// Height for the retry banner: 1 if present, 0 if absent.
pub fn banner_height(banner: &Option<String>) -> u16 {
    if banner.is_some() { 1 } else { 0 }
}
