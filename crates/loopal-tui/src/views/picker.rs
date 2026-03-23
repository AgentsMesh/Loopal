use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::PickerState;

/// Render a full-screen picker sub-page.
///
/// Layout (top to bottom):
///   - Title bar with border
///   - Filter input line
///   - Scrollable list of items
///   - Hint bar at the bottom
pub fn render_picker(f: &mut Frame, picker: &PickerState, area: Rect) {
    // Clear background
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", picker.title))
        .title_bottom(build_hint_bar(picker))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Render thinking indicator overlaid on the top-right of the border
    render_thinking_indicator(f, picker, area);

    if inner.height < 3 {
        return;
    }

    // Row 0: filter input
    let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let filter_label = Span::styled(" Filter: ", Style::default().fg(Color::DarkGray));
    let filter_text = Span::styled(&picker.filter, Style::default().fg(Color::White).bold());
    let cursor_hint = Span::styled("█", Style::default().fg(Color::DarkGray));
    let filter_line = Line::from(vec![filter_label, filter_text, cursor_hint]);
    f.render_widget(Paragraph::new(filter_line), filter_area);

    // Row 1: separator
    let sep_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    let sep = "─".repeat(inner.width as usize);
    f.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::DarkGray)),
        sep_area,
    );

    // Remaining rows: item list
    let list_y = inner.y + 2;
    let list_height = inner.height.saturating_sub(2) as usize;
    let filtered = picker.filtered_items();

    if filtered.is_empty() {
        let empty_area = Rect::new(inner.x, list_y, inner.width, 1);
        f.render_widget(
            Paragraph::new("  No matching items").style(Style::default().fg(Color::DarkGray)),
            empty_area,
        );
        return;
    }

    // Scroll so that the selected item is always visible
    let scroll_offset = if picker.selected >= list_height {
        picker.selected - list_height + 1
    } else {
        0
    };

    for (i, item) in filtered
        .iter()
        .skip(scroll_offset)
        .take(list_height)
        .enumerate()
    {
        let abs_idx = scroll_offset + i;
        let is_selected = abs_idx == picker.selected;

        let indicator = if is_selected { " ▸ " } else { "   " };

        let line = Line::from(vec![
            Span::styled(indicator, Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{:<30}", item.label),
                if is_selected {
                    Style::default().fg(Color::Cyan).bold()
                } else {
                    Style::default().fg(Color::White)
                },
            ),
            Span::styled(&item.description, Style::default().fg(Color::DarkGray)),
        ]);

        let row_area = Rect::new(inner.x, list_y + i as u16, inner.width, 1);

        let bg = if is_selected {
            Style::default().bg(Color::Rgb(40, 40, 40))
        } else {
            Style::default()
        };

        f.render_widget(Paragraph::new(line).style(bg), row_area);
    }
}

/// Render `Thinking: ◀ High ▶` overlaid on the top-right border of the picker.
fn render_thinking_indicator(f: &mut Frame, picker: &PickerState, area: Rect) {
    if picker.thinking_options.is_empty() || area.width < 30 {
        return;
    }
    let label = picker
        .thinking_options
        .get(picker.thinking_selected)
        .map(|o| o.label)
        .unwrap_or("Auto");
    let indicator = Line::from(vec![
        Span::styled(" Thinking: ", Style::default().fg(Color::DarkGray)),
        Span::styled("◀ ", Style::default().fg(Color::Magenta)),
        Span::styled(label, Style::default().fg(Color::Magenta).bold()),
        Span::styled(" ▶ ", Style::default().fg(Color::Magenta)),
    ]);
    // Estimate width: " Thinking: ◀ {label} ▶ " ~ 18 + label.len()
    let w = (18 + label.len()).min(area.width as usize - 2) as u16;
    let x = area.x + area.width - w - 1; // -1 for right border
    let indicator_area = Rect::new(x, area.y, w, 1);
    f.render_widget(Paragraph::new(indicator), indicator_area);
}

/// Build the bottom hint bar text, including thinking hint when applicable.
fn build_hint_bar(picker: &PickerState) -> Line<'static> {
    if picker.thinking_options.is_empty() {
        return Line::from(" Esc to go back ");
    }
    Line::from(vec![
        Span::raw(" Esc to go back  "),
        Span::styled("◀▶", Style::default().fg(Color::Magenta)),
        Span::raw(" thinking  "),
        Span::styled("↑↓", Style::default().fg(Color::Cyan)),
        Span::raw(" model  "),
        Span::styled("⏎", Style::default().fg(Color::Green)),
        Span::raw(" confirm "),
    ])
}
