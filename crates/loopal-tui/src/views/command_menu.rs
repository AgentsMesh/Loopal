use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear};

use crate::app::AutocompleteState;

const MAX_MENU_ITEMS: usize = 8;

pub fn render_command_menu(f: &mut Frame, ac: &AutocompleteState, input_area: Rect) {
    if ac.matches.is_empty() {
        return;
    }

    let visible = ac.matches.len().min(MAX_MENU_ITEMS);
    let menu_height = visible as u16 + 2;

    let y = input_area.y.saturating_sub(menu_height);
    let menu_area = Rect::new(input_area.x, y, input_area.width, menu_height);

    f.render_widget(Clear, menu_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Commands ")
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(menu_area);
    f.render_widget(block, menu_area);

    let scroll_offset = scroll_offset_for_selection(ac.selected, visible);

    for (i, entry) in ac
        .matches
        .iter()
        .skip(scroll_offset)
        .take(visible)
        .enumerate()
    {
        let abs_idx = scroll_offset + i;
        let is_selected = abs_idx == ac.selected;

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

pub fn scroll_offset_for_selection(selected: usize, visible: usize) -> usize {
    if visible == 0 {
        return 0;
    }
    if selected >= visible {
        selected + 1 - visible
    } else {
        0
    }
}
