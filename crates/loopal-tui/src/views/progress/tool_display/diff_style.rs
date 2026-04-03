//! Diff visualization primitives — colors, line rendering, and fold indicator.
//!
//! Shared by all file-editing tool renderers (Edit, MultiEdit, Write, ApplyPatch).

use ratatui::prelude::*;

/// Max diff lines before folding (shared across all diff-capable renderers).
pub(crate) const DIFF_MAX_LINES: usize = 8;

// ── Semantic colors ──

pub(crate) fn removed_style() -> Style {
    Style::default().fg(Color::Rgb(220, 80, 80))
}

pub(crate) fn added_style() -> Style {
    Style::default().fg(Color::Rgb(80, 200, 80))
}

pub(crate) fn header_style() -> Style {
    Style::default().fg(Color::Rgb(130, 170, 220))
}

// ── Line-level primitives ──

/// A single removed line: `    - {text}` in red.
pub(crate) fn removed_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(format!("    - {text}"), removed_style()))
}

/// A single added line: `    + {text}` in green.
pub(crate) fn added_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(format!("    + {text}"), added_style()))
}

// ── Composite rendering ──

/// Result of rendering a single old→new replacement diff.
pub(crate) struct DiffResult {
    pub added: usize,
    pub removed: usize,
    pub lines: Vec<Line<'static>>,
}

/// Render diff lines for a single old→new replacement.
///
/// At most `max_lines` diff lines are emitted; the caller handles folding.
pub(crate) fn render_diff_lines(old: &str, new: &str, max_lines: usize) -> DiffResult {
    let old_lines: Vec<&str> = if old.is_empty() {
        Vec::new()
    } else {
        old.lines().collect()
    };
    let new_lines: Vec<&str> = if new.is_empty() {
        Vec::new()
    } else {
        new.lines().collect()
    };

    let mut lines = Vec::new();
    let mut shown = 0;

    for line in &old_lines {
        if shown >= max_lines {
            break;
        }
        lines.push(removed_line(line));
        shown += 1;
    }
    for line in &new_lines {
        if shown >= max_lines {
            break;
        }
        lines.push(added_line(line));
        shown += 1;
    }
    DiffResult {
        added: new_lines.len(),
        removed: old_lines.len(),
        lines,
    }
}

/// Fold indicator: "    … +N lines" in dim style.
pub(crate) fn fold_indicator(hidden: usize) -> Line<'static> {
    Line::from(Span::styled(
        format!("    … +{hidden} lines"),
        super::dim_style(),
    ))
}
