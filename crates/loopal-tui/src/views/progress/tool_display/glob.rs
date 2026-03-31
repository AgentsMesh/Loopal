//! Glob tool rendering.

use ratatui::prelude::*;

use loopal_session::types::SessionToolCall;

use super::{EXPAND_MAX_LINES, expand_output, output_first_line, output_style};

/// Header detail: glob pattern.
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input
        .get("pattern")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Body: expand first N file names.
pub fn render_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let Some(ref result) = tc.result else {
        return vec![output_first_line("no matches")];
    };
    if result.trim().is_empty() {
        return vec![output_first_line("no matches")];
    }
    expand_output(result, EXPAND_MAX_LINES, output_style())
}
