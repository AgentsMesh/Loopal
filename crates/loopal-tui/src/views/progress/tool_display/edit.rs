//! Edit tool rendering — shows inline diff with -/+ markers.

use ratatui::prelude::*;

use loopal_session::types::SessionToolCall;

use super::output_first_line;

/// Max diff lines before folding.
const DIFF_MAX_LINES: usize = 8;

/// Header detail: file path.
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Body: show summary + inline diff content.
pub fn render_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let Some(ref input) = tc.tool_input else {
        return vec![output_first_line("edited")];
    };
    let old = input
        .get("old_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let new = input
        .get("new_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");

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
    let removed = old_lines.len();
    let added = new_lines.len();

    let mut lines = Vec::new();

    // Summary line
    let summary = format_summary(added, removed);
    lines.push(output_first_line(&summary));

    // Diff body: removed lines (red), then added lines (green)
    let red = Style::default().fg(Color::Rgb(220, 80, 80));
    let green = Style::default().fg(Color::Rgb(80, 200, 80));
    let dim = Style::default().fg(Color::Rgb(100, 105, 115));

    let total_diff = removed + added;
    let mut shown = 0;

    for line in &old_lines {
        if shown >= DIFF_MAX_LINES {
            break;
        }
        lines.push(Line::from(Span::styled(format!("    - {line}"), red)));
        shown += 1;
    }
    for line in &new_lines {
        if shown >= DIFF_MAX_LINES {
            break;
        }
        lines.push(Line::from(Span::styled(format!("    + {line}"), green)));
        shown += 1;
    }
    if total_diff > shown {
        lines.push(Line::from(Span::styled(
            format!("    … +{} lines", total_diff - shown),
            dim,
        )));
    }

    lines
}

fn format_summary(added: usize, removed: usize) -> String {
    match (added, removed) {
        (0, r) => format!("Removed {r} line{}", plural(r)),
        (a, 0) => format!("Added {a} line{}", plural(a)),
        (a, r) => format!("Added {a} line{}, removed {r} line{}", plural(a), plural(r)),
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
