//! Tool output formatting primitives — styles, indented lines, and expand-with-fold.

use ratatui::prelude::*;

/// Standard style for tool output text — light enough for dark-mode readability.
pub(crate) fn output_style() -> Style {
    Style::default().fg(Color::Rgb(155, 160, 170))
}

/// Dimmed style for secondary info (elapsed time, fold counts, etc.).
pub(crate) fn dim_style() -> Style {
    Style::default().fg(Color::Rgb(100, 105, 115))
}

/// Expand output up to `max_lines`, fold the rest.
pub(crate) fn expand_output(content: &str, max_lines: usize, style: Style) -> Vec<Line<'static>> {
    let all: Vec<&str> = content.lines().collect();
    let total = all.len();
    let mut lines = Vec::new();

    for (i, text) in all.iter().take(max_lines).enumerate() {
        let prefix = if i == 0 { "  ⎿ " } else { "    " };
        lines.push(Line::from(Span::styled(format!("{prefix}{text}"), style)));
    }

    if total > max_lines {
        lines.push(Line::from(Span::styled(
            format!("    … +{} lines", total - max_lines),
            dim_style(),
        )));
    }
    lines
}

/// Single output line with ⎿ prefix.
pub(crate) fn output_first_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(format!("  ⎿ {text}"), output_style()))
}
