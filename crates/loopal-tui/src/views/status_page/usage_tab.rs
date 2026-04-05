//! Usage tab — token metrics and session statistics.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::app::StatusPageState;

/// A labeled metric row.
struct MetricRow {
    label: &'static str,
    value: String,
    style: Style,
}

/// Render the Usage tab. Returns the total row count for scroll clamping.
pub(super) fn render_usage_tab(f: &mut Frame, state: &StatusPageState, area: Rect) -> usize {
    let u = &state.usage;

    let ctx_display = if u.context_window > 0 {
        format!("{}k / {}k", u.context_used / 1000, u.context_window / 1000)
    } else {
        format_tokens(u.context_used)
    };

    let rows = [
        metric("Input Tokens", format_tokens(u.input_tokens), val_style()),
        metric("Output Tokens", format_tokens(u.output_tokens), val_style()),
        separator_row(),
        metric("Context Window", ctx_display, val_style()),
        separator_row(),
        metric("Turns", u.turn_count.to_string(), val_style()),
        metric("Tool Calls", u.tool_count.to_string(), val_style()),
    ];

    let scroll = state.active_scroll();
    let visible = area.height as usize;
    // Clamp: when all rows fit on screen, no scrolling needed.
    let scroll = scroll.min(rows.len().saturating_sub(visible));

    for (i, row) in rows.iter().skip(scroll).take(visible).enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        let row_area = Rect::new(area.x, y, area.width, 1);

        if row.label == "---" {
            let sep = "\u{2500}".repeat((area.width as usize).min(40));
            f.render_widget(
                Paragraph::new(format!("  {sep}"))
                    .style(Style::default().fg(Color::Rgb(50, 50, 50))),
                row_area,
            );
        } else {
            let line = Line::from(vec![
                Span::styled(
                    format!("  {:<20}", row.label),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(&row.value, row.style),
            ]);
            f.render_widget(Paragraph::new(line), row_area);
        }
    }
    rows.len()
}

fn metric(label: &'static str, value: String, style: Style) -> MetricRow {
    MetricRow {
        label,
        value,
        style,
    }
}

fn separator_row() -> MetricRow {
    MetricRow {
        label: "---",
        value: String::new(),
        style: Style::default(),
    }
}

fn val_style() -> Style {
    Style::default().fg(Color::White)
}

/// Format token count with thousand separators.
fn format_tokens(n: u32) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
