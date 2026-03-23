//! Edit tool rendering.

use ratatui::prelude::*;

use loopal_session::types::DisplayToolCall;

use super::output_first_line;

/// Header detail: file path.
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Body: show added/removed line counts.
pub fn render_body(tc: &DisplayToolCall) -> Vec<Line<'static>> {
    if let Some(ref input) = tc.tool_input {
        let old = input.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
        let new = input.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
        let removed = old.lines().count();
        let added = new.lines().count();
        let summary = match (added, removed) {
            (0, r) => format!("Removed {r} lines"),
            (a, 0) => format!("Added {a} lines"),
            (a, r) => format!("Added {a} lines, removed {r} lines"),
        };
        return vec![output_first_line(&summary)];
    }
    vec![output_first_line("edited")]
}
