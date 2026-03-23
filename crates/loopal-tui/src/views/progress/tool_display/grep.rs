//! Grep tool rendering.

use ratatui::prelude::*;

use loopal_session::types::DisplayToolCall;

use super::{expand_output, output_first_line, EXPAND_MAX_LINES};

/// Header detail: search pattern.
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input.get("pattern").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Body: expand first N matching lines.
pub fn render_body(tc: &DisplayToolCall) -> Vec<Line<'static>> {
    let Some(ref result) = tc.result else { return vec![output_first_line("no matches")] };
    if result.trim().is_empty() {
        return vec![output_first_line("no matches")];
    }
    expand_output(result, EXPAND_MAX_LINES, Style::default().fg(Color::DarkGray))
}
