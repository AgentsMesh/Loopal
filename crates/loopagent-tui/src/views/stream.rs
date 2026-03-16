use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Render a streaming text component showing partial content being received.
pub fn render_stream(f: &mut Frame, text: &str, area: Rect) {
    if text.is_empty() {
        return;
    }

    let block = Block::default()
        .borders(Borders::NONE);

    let lines: Vec<Line> = text.lines().map(|l| Line::from(l.to_string())).collect();

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::Cyan));

    f.render_widget(paragraph, area);
}
