//! Breadcrumb bar — shows navigation path when viewing a sub-agent.
//!
//! `root ▸ researcher                               ESC to return`

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Render a 1-line breadcrumb showing the viewed agent path.
pub fn render_breadcrumb(f: &mut Frame, agent_name: &str, area: Rect) {
    let hint = "ESC to return ";
    // Truncate agent name if it would overflow the breadcrumb bar
    let prefix_len = " root ▸ ".len();
    let max_name = (area.width as usize)
        .saturating_sub(prefix_len)
        .saturating_sub(hint.len())
        .saturating_sub(2); // padding
    let display_name: String = if agent_name.len() > max_name && max_name > 1 {
        let t: String = agent_name.chars().take(max_name - 1).collect();
        format!("{t}…")
    } else {
        agent_name.to_string()
    };
    let path = format!(" root ▸ {display_name}");
    let fill_len = (area.width as usize)
        .saturating_sub(path.len())
        .saturating_sub(hint.len());
    let line = Line::from(vec![
        Span::styled(path, Style::default().fg(Color::Cyan).bold()),
        Span::raw(" ".repeat(fill_len.max(1))),
        Span::styled(hint, Style::default().fg(Color::DarkGray)),
    ]);
    let bg = Style::default().bg(Color::Rgb(20, 30, 40));
    f.render_widget(Paragraph::new(line).style(bg), area);
}
