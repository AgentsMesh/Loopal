use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Render the plan mode indicator banner.
pub fn render_plan_indicator(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(" Plan Mode ");

    let text = Paragraph::new(Line::from(Span::styled(
        " PLAN MODE - Read-only, no tool execution ",
        Style::default().fg(Color::Magenta).bold(),
    )))
    .block(block);

    f.render_widget(text, area);
}
