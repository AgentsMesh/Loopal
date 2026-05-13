use ratatui::prelude::*;

use loopal_view_state::ToolInvocation;

use super::{EXPAND_MAX_LINES, expand_output, output_first_line, output_style};

pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input
        .get("pattern")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub fn render_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let Some(content) = tc.state.outcome().map(|o| o.content()) else {
        return vec![output_first_line("no matches")];
    };
    if content.trim().is_empty() {
        return vec![output_first_line("no matches")];
    }
    expand_output(content, EXPAND_MAX_LINES, output_style())
}
