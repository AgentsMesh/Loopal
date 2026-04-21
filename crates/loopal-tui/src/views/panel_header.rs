//! Section header for the panel zone — `━━ Title (count) ━━━━…`.
//!
//! Rendered only when ≥2 panels are visible, acting as both label and
//! separator. The active panel's header is highlighted (Cyan + bold);
//! inactive headers use dim colors so focus is visually unambiguous.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::DIM_SEPARATOR;
use super::text_width::display_width;

const HEADER_PREFIX: &str = "━━";

/// Render a single-line section header at `area`.
///
/// - `title`: short label (e.g. "Tasks", "Background").
/// - `count`: item count shown in parentheses; omitted when 0.
/// - `focused`: true when this panel is the active focus target.
pub fn render_section_header(f: &mut Frame, title: &str, count: usize, focused: bool, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (title_fg, decor_fg) = if focused {
        (Color::Cyan, Color::Cyan)
    } else {
        (Color::Gray, DIM_SEPARATOR)
    };
    let title_style = if focused {
        Style::default().fg(title_fg).bold()
    } else {
        Style::default().fg(title_fg)
    };

    let label = if count > 0 {
        format!(" {title} ({count}) ")
    } else {
        format!(" {title} ")
    };
    let total = area.width as usize;
    let left_w = display_width(HEADER_PREFIX);
    let label_w = display_width(&label);
    let right_w = total.saturating_sub(left_w + label_w);
    let right: String = "━".repeat(right_w);

    let line = Line::from(vec![
        Span::styled(HEADER_PREFIX.to_string(), Style::default().fg(decor_fg)),
        Span::styled(label, title_style),
        Span::styled(right, Style::default().fg(decor_fg)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}
