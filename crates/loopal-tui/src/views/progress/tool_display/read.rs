//! Read tool rendering.

use ratatui::prelude::*;

use loopal_session::types::SessionToolCall;

use super::output_first_line;

/// Header detail: file path.
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Body: just show line count (like Claude Code — no content expansion).
pub fn render_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let line_count = tc.result.as_deref().map_or(0, |r| r.lines().count());
    vec![output_first_line(&format!("Read {line_count} lines"))]
}
