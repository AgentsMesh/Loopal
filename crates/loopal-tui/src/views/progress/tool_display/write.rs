//! Write tool rendering — shows bytes written + content preview.

use ratatui::prelude::*;

use loopal_session::types::SessionToolCall;

use super::diff_style::{self, DIFF_MAX_LINES};
use super::output_first_line;

/// Header detail: file path.
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Body: show bytes written + content preview (green `+` lines via diff_style).
pub fn render_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let msg = tc
        .metadata
        .as_ref()
        .and_then(|m| m.get("bytes_written"))
        .and_then(|v| v.as_u64())
        .map(format_bytes)
        .or_else(|| {
            // Fallback: parse legacy string format for backward compat
            tc.result.as_deref().and_then(|r| {
                r.trim()
                    .strip_prefix("Successfully wrote ")
                    .and_then(|s| s.split(' ').next())
                    .and_then(|n| n.parse::<u64>().ok())
                    .map(format_bytes)
            })
        })
        .unwrap_or_else(|| "written".to_string());

    let mut lines = vec![output_first_line(&msg)];

    // Content preview: treat as all-new (old="", new=content)
    if let Some(content) = tc
        .tool_input
        .as_ref()
        .and_then(|i| i.get("content"))
        .and_then(|v| v.as_str())
    {
        let diff = diff_style::render_diff_lines("", content, DIFF_MAX_LINES);
        lines.extend(diff.lines);
        if diff.added > DIFF_MAX_LINES {
            lines.push(diff_style::fold_indicator(diff.added - DIFF_MAX_LINES));
        }
    }

    lines
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B written")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB written", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB written", bytes as f64 / (1024.0 * 1024.0))
    }
}
