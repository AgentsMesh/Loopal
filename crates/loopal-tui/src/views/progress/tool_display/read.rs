use ratatui::prelude::*;

use loopal_view_state::ToolInvocation;

use super::output_first_line;

pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub fn render_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let line_count = tc
        .state
        .outcome()
        .map(|o| o.content().lines().count())
        .unwrap_or(0);
    vec![output_first_line(&format!("Read {line_count} lines"))]
}
