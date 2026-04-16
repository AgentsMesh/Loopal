//! Action menu overlay for MCP server operations (disconnect / reconnect).

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::McpPageState;

pub fn render(f: &mut Frame, state: &McpPageState, area: Rect) {
    let Some(menu) = &state.action_menu else {
        return;
    };
    let menu_h = menu.options.len() as u16 + 2;
    let menu_w: u16 = 22;
    let x = area.x + (area.width.saturating_sub(menu_w)) / 2;
    let y = area.y + (area.height.saturating_sub(menu_h)) / 2;
    let menu_area = Rect::new(x, y, menu_w.min(area.width), menu_h.min(area.height));

    f.render_widget(Clear, menu_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(Span::styled(
            " Action ",
            Style::default().fg(Color::Cyan).bold(),
        )))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(menu_area);
    f.render_widget(block, menu_area);

    for (i, action) in menu.options.iter().enumerate() {
        let row_y = inner.y + i as u16;
        if row_y >= inner.y + inner.height {
            break;
        }
        let is_sel = i == menu.cursor;
        let prefix = if is_sel { "\u{25b8} " } else { "  " };
        let style = if is_sel {
            Style::default().fg(Color::White).bold()
        } else {
            Style::default().fg(Color::Gray)
        };
        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(action.label(), style),
        ]);
        f.render_widget(
            Paragraph::new(line),
            Rect::new(inner.x, row_y, inner.width, 1),
        );
    }
}

pub fn hint_bar() -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled("\u{2191}/\u{2193}", Style::default().fg(Color::Cyan)),
        Span::raw(" select  "),
        Span::styled("Enter", Style::default().fg(Color::Green)),
        Span::raw(" confirm  "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(" cancel "),
    ])
}
