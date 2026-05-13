use ratatui::prelude::*;

use loopal_view_state::ToolInvocation;

use super::diff_style::{self, DIFF_MAX_LINES};
use super::output_first_line;

pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    let patch = input.get("patch")?.as_str()?;
    let n = patch.lines().filter(|l| l.starts_with("*** ")).count();
    Some(format!("{n} file(s)"))
}

pub fn render_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let Some(patch) = tc
        .input
        .as_ref()
        .and_then(|i| i.get("patch"))
        .and_then(|v| v.as_str())
    else {
        return vec![output_first_line("patch applied")];
    };

    let hdr = diff_style::header_style();
    let mut diff_lines: Vec<Line<'static>> = Vec::new();
    let mut shown = 0usize;
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut files = 0usize;

    for text in patch.lines() {
        if text.starts_with("*** ") {
            files += 1;
            if shown < DIFF_MAX_LINES {
                diff_lines.push(Line::from(Span::styled(format!("    {text}"), hdr)));
                shown += 1;
            }
        } else if let Some(rest) = text.strip_prefix('-') {
            removed += 1;
            if shown < DIFF_MAX_LINES {
                diff_lines.push(diff_style::removed_line(rest));
                shown += 1;
            }
        } else if let Some(rest) = text.strip_prefix('+') {
            added += 1;
            if shown < DIFF_MAX_LINES {
                diff_lines.push(diff_style::added_line(rest));
                shown += 1;
            }
        }
    }

    let summary = format_patch_summary(files, added, removed);
    let mut lines = vec![output_first_line(&summary)];
    lines.extend(diff_lines);

    let total = added + removed + files;
    if total > DIFF_MAX_LINES {
        lines.push(diff_style::fold_indicator(total - DIFF_MAX_LINES));
    }
    lines
}

fn format_patch_summary(files: usize, added: usize, removed: usize) -> String {
    let mut parts = vec![format!("{files} file(s)")];
    if added > 0 {
        parts.push(format!("+{added}"));
    }
    if removed > 0 {
        parts.push(format!("-{removed}"));
    }
    parts.join(", ")
}
