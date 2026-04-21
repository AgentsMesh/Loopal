/// Dim dashed separator line — replaces heavy `Borders::ALL` visual separation.
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::DIM_SEPARATOR;

/// Render a dim horizontal dashed line across the full width.
///
/// Pattern `─ ─ ─` repeats to fill the area. Ratatui's Paragraph
/// handles column-level truncation at area.width, so we just need
/// to generate enough repeats to cover the width.
pub fn render_separator(f: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    // Each "─ " is 2 display columns. Generate enough repeats.
    let repeats = area.width as usize / 2 + 1;
    let pattern: String = "─ ".repeat(repeats);
    let line = Line::from(Span::styled(pattern, Style::default().fg(DIM_SEPARATOR)));
    f.render_widget(Paragraph::new(line), area);
}
