//! Rewind turn picker — renders a list of user turns for the user to select.
//!
//! ```text
//! ┌─ Rewind to Turn ─────────────────────────────────┐
//! │                                                   │
//! │  ▸ [3] Fix authentication bug in login flow...    │
//! │    [2] Add user profile page with avatar...       │
//! │    [1] Initialize project structure and set...    │
//! │                                                   │
//! │  ↑↓ Navigate  Enter Confirm  Esc Cancel           │
//! │  Selected turn and everything after it will be    │
//! │  removed from the conversation.                   │
//! └───────────────────────────────────────────────────┘
//! ```

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::RewindPickerState;

pub fn render_rewind_picker(f: &mut Frame, state: &RewindPickerState, area: Rect) {
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Rewind to Turn ")
        .title_bottom(" ↑↓ Navigate  Enter Confirm  Esc Cancel ")
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 3 {
        return;
    }

    // Hint line at top
    let hint_area = Rect::new(inner.x, inner.y, inner.width, 1);
    f.render_widget(
        Paragraph::new("  Select a turn — it and everything after will be removed.")
            .style(Style::default().fg(Color::DarkGray)),
        hint_area,
    );

    // Separator
    let sep_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    let sep = "─".repeat(inner.width as usize);
    f.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
        sep_area,
    );

    // Turn list
    let list_y = inner.y + 2;
    let list_height = inner.height.saturating_sub(2) as usize;

    if state.turns.is_empty() {
        let empty_area = Rect::new(inner.x, list_y, inner.width, 1);
        f.render_widget(
            Paragraph::new("  No turns available")
                .style(Style::default().fg(Color::DarkGray)),
            empty_area,
        );
        return;
    }

    let scroll_offset = if state.selected >= list_height {
        state.selected - list_height + 1
    } else {
        0
    };

    for (i, item) in state
        .turns
        .iter()
        .skip(scroll_offset)
        .take(list_height)
        .enumerate()
    {
        let abs_idx = scroll_offset + i;
        let is_selected = abs_idx == state.selected;
        let indicator = if is_selected { " ▸ " } else { "   " };
        let turn_num = item.turn_index + 1; // 1-based for display

        let line = Line::from(vec![
            Span::styled(indicator, Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("[{turn_num}] "),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                &item.preview,
                if is_selected {
                    Style::default().fg(Color::White).bold()
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]);

        let row_area = Rect::new(inner.x, list_y + i as u16, inner.width, 1);
        let bg = if is_selected {
            Style::default().bg(Color::Rgb(50, 40, 20))
        } else {
            Style::default()
        };
        f.render_widget(Paragraph::new(line).style(bg), row_area);
    }
}
