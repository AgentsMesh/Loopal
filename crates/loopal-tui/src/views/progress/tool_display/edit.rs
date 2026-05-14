use ratatui::prelude::*;

use loopal_view_state::ToolInvocation;

use super::diff_style::{self, DIFF_MAX_LINES};
use super::output_first_line;

pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub fn render_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let Some(ref input) = tc.input else {
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

    let diff = diff_style::render_diff_lines(old, new, DIFF_MAX_LINES);
    let total = diff.added + diff.removed;
    let mut lines = vec![output_first_line(&format_summary(diff.added, diff.removed))];
    lines.extend(diff.lines);
    if total > DIFF_MAX_LINES {
        lines.push(diff_style::fold_indicator(total - DIFF_MAX_LINES));
    }
    lines
}

pub fn render_multi_edit_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let edits = tc
        .input
        .as_ref()
        .and_then(|i| i.get("edits"))
        .and_then(|v| v.as_array());
    let Some(edits) = edits else {
        return vec![output_first_line("edited")];
    };

    let mut all_diff: Vec<Line<'static>> = Vec::new();
    let (mut total_added, mut total_removed) = (0usize, 0usize);
    let mut budget = DIFF_MAX_LINES;

    for edit in edits {
        let old = edit
            .get("old_string")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let new = edit
            .get("new_string")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let diff = diff_style::render_diff_lines(old, new, budget);
        total_added += diff.added;
        total_removed += diff.removed;
        budget = budget.saturating_sub(diff.lines.len());
        all_diff.extend(diff.lines);
    }

    let summary = format!(
        "{} edit(s): {}",
        edits.len(),
        format_summary(total_added, total_removed)
    );
    let mut lines = vec![output_first_line(&summary)];
    lines.extend(all_diff);
    let total = total_added + total_removed;
    if total > DIFF_MAX_LINES {
        lines.push(diff_style::fold_indicator(total - DIFF_MAX_LINES));
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
