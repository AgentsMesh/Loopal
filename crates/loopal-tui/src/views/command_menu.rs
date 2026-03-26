use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear};

use crate::app::AutocompleteState;

/// Maximum visible items in the autocomplete dropdown.
const MAX_MENU_ITEMS: usize = 8;

/// Render the floating command autocomplete menu above the input area.
pub fn render_command_menu(f: &mut Frame, ac: &AutocompleteState, input_area: Rect) {
    if ac.matches.is_empty() {
        return;
    }

    let item_count = ac.matches.len().min(MAX_MENU_ITEMS) as u16;
    // +2 for border top/bottom
    let menu_height = item_count + 2;

    // Position just above the input area, same width
    let y = input_area.y.saturating_sub(menu_height);
    let menu_area = Rect::new(input_area.x, y, input_area.width, menu_height);

    // Clear background
    f.render_widget(Clear, menu_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Commands ")
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(menu_area);
    f.render_widget(block, menu_area);

    // Render each command line
    for (i, entry) in ac.matches.iter().take(item_count as usize).enumerate() {
        let is_selected = i == ac.selected;

        let indicator = if is_selected { "▸" } else { " " };

        let line = Line::from(vec![
            Span::styled(indicator, Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{:<12}", entry.name),
                if is_selected {
                    Style::default().fg(Color::Cyan).bold()
                } else {
                    Style::default().fg(Color::Cyan)
                },
            ),
            Span::styled(&entry.description, Style::default().fg(Color::DarkGray)),
        ]);

        let line_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);

        let bg = if is_selected {
            Style::default().bg(Color::Rgb(40, 40, 40))
        } else {
            Style::default()
        };

        f.render_widget(ratatui::widgets::Paragraph::new(line).style(bg), line_area);
    }
}
