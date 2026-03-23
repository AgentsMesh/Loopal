//! Write tool rendering.

use ratatui::prelude::*;

use loopal_session::types::DisplayToolCall;

use super::output_first_line;

/// Header detail: file path.
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    input.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Body: show bytes written.
pub fn render_body(tc: &DisplayToolCall) -> Vec<Line<'static>> {
    let msg = tc
        .result
        .as_deref()
        .and_then(|r| {
            r.trim()
                .strip_prefix("Successfully wrote ")
                .and_then(|s| s.split(' ').next())
                .and_then(|n| n.parse::<u64>().ok())
                .map(format_bytes)
        })
        .unwrap_or_else(|| "written".to_string());
    vec![output_first_line(&msg)]
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
