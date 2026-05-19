use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render_compact_banner(f: &mut Frame, message: &str, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default().fg(Color::Cyan).bg(Color::Rgb(15, 30, 50));
    let line = Line::from(vec![
        Span::styled("▼ ", style.bold()),
        Span::styled(message.to_string(), style),
    ]);
    f.render_widget(Paragraph::new(line).style(style), area);
}

pub fn banner_height(banner: &Option<String>) -> u16 {
    if banner.is_some() { 1 } else { 0 }
}
