//! Config tab — searchable settings key-value list.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::app::StatusPageState;

/// Render the Config tab. Returns the total filtered row count for scroll clamping.
pub(super) fn render_config_tab(f: &mut Frame, state: &StatusPageState, area: Rect) -> usize {
    if area.height < 3 {
        return state.filtered_config().len();
    }

    // Row 0: filter input
    let filter_area = Rect::new(area.x, area.y, area.width, 1);
    let filter_line = Line::from(vec![
        Span::styled("  Filter: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&state.filter, Style::default().fg(Color::White).bold()),
        Span::styled("\u{2588}", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(filter_line), filter_area);

    // Row 1: separator
    let sep_area = Rect::new(area.x, area.y + 1, area.width, 1);
    let sep = "\u{2500}".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(sep).style(Style::default().fg(Color::Rgb(50, 50, 50))),
        sep_area,
    );

    // Rows 2+: config entries
    let list_y = area.y + 2;
    let list_height = area.height.saturating_sub(2) as usize;
    let filtered = state.filtered_config();
    let total = filtered.len();

    if filtered.is_empty() {
        let empty_area = Rect::new(area.x, list_y, area.width, 1);
        f.render_widget(
            Paragraph::new("  No matching settings").style(Style::default().fg(Color::DarkGray)),
            empty_area,
        );
        return total;
    }

    let scroll = state.active_scroll();
    // Clamp: prevent scrolling past the point where last row is at bottom.
    let scroll = scroll.min(filtered.len().saturating_sub(list_height));

    for (i, entry) in filtered.iter().skip(scroll).take(list_height).enumerate() {
        let y = list_y + i as u16;
        if y >= area.y + area.height {
            break;
        }

        // Key column: fixed display-width, right-padded. Truncate if wider.
        let key_width = 36.min(area.width as usize / 2);
        let key_text = pad_to_width(&entry.key, key_width);
        let val_text = &entry.value;

        let line = Line::from(vec![
            Span::styled(format!("  {key_text}"), Style::default().fg(Color::Cyan)),
            Span::styled(val_text, Style::default().fg(Color::White)),
        ]);

        let row_area = Rect::new(area.x, y, area.width, 1);
        f.render_widget(Paragraph::new(line), row_area);
    }
    total
}

/// Truncate or pad a string to exactly `target_width` terminal columns.
fn pad_to_width(s: &str, target_width: usize) -> String {
    let w = UnicodeWidthStr::width(s);
    if w <= target_width {
        // Pad with spaces to fill remaining columns.
        let padding = target_width - w;
        let mut out = s.to_string();
        for _ in 0..padding {
            out.push(' ');
        }
        out
    } else {
        // Truncate to fit, appending "…" (1 column wide).
        let mut out = String::new();
        let mut used = 0;
        let limit = target_width.saturating_sub(1); // reserve 1 col for "…"
        for ch in s.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if used + cw > limit {
                break;
            }
            out.push(ch);
            used += cw;
        }
        out.push('\u{2026}');
        // Pad if truncation left a gap (e.g. wide char didn't fit).
        let remaining = target_width.saturating_sub(used + 1);
        for _ in 0..remaining {
            out.push(' ');
        }
        out
    }
}
