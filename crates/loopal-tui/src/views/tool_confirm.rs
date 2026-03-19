use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

/// Render the tool confirmation popup.
pub fn render_tool_confirm(
    f: &mut Frame,
    name: &str,
    input: &serde_json::Value,
    area: Rect,
) {
    // Center a popup at 60% width, 50% height
    let popup_width = (area.width * 60 / 100).clamp(30, 80);
    let popup_height = (area.height * 50 / 100).clamp(8, 20);
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the background
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Tool: {} ", name))
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Format input JSON
    let json_str = serde_json::to_string_pretty(input).unwrap_or_else(|_| input.to_string());
    let mut lines: Vec<Line> = Vec::new();
    for line in json_str.lines().take((inner.height as usize).saturating_sub(2)) {
        lines.push(Line::from(line.to_string()));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[y] Allow  [n] Deny",
        Style::default().fg(Color::Yellow).bold(),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}
