use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render_inline_banner(
    f: &mut Frame,
    prefix: &str,
    message: &str,
    fg: Color,
    bg: Color,
    area: Rect,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default().fg(fg).bg(bg);
    let line = Line::from(vec![
        Span::styled(prefix.to_string(), style.bold()),
        Span::styled(message.to_string(), style),
    ]);
    f.render_widget(Paragraph::new(line).style(style), area);
}

pub fn banner_height(banner: &Option<String>) -> u16 {
    if banner.is_some() { 1 } else { 0 }
}
