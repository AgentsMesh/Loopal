//! Agent tool rendering: header detail + body (running / success).

use ratatui::prelude::*;

use loopal_view_state::SessionToolCall;

use super::{EXPAND_MAX_LINES, dim_style, expand_output, output_first_line, output_style};

/// Header detail: combine agent name + description.
///
/// - Both present: `"researcher — analyze the codebase"`
/// - Name only:    `"researcher"`
/// - Desc only:    `"analyze the codebase"`
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    let name = input.get("name").and_then(|v| v.as_str());
    let desc = input.get("description").and_then(|v| v.as_str());
    match (name, desc) {
        (Some(n), Some(d)) => Some(format!("{n} — {d}")),
        (Some(n), None) => Some(n.to_string()),
        (None, Some(d)) => Some(d.to_string()),
        (None, None) => None,
    }
}

/// Running Agent: show sub-agent's last tool status + elapsed time.
pub fn render_running_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let dim = output_style();
    let elapsed = tc
        .started_at
        .map(|t| format!("{:.1}s", t.elapsed().as_secs_f64()))
        .unwrap_or_else(|| "...".to_string());

    if let Some(ref tail) = tc.progress_tail {
        let trimmed = tail.trim();
        if !trimmed.is_empty() {
            return vec![
                Line::from(Span::styled(format!("  ⎿ {trimmed}"), dim)),
                Line::from(Span::styled(format!("    {elapsed}"), dim_style())),
            ];
        }
    }

    vec![Line::from(Span::styled(
        format!("  ⎿ Working… ({elapsed})"),
        dim,
    ))]
}

/// Completed Agent: expand result output.
pub fn render_success_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let Some(ref result) = tc.result else {
        return vec![output_first_line("(No output)")];
    };
    if result.trim().is_empty() {
        return vec![output_first_line("(No output)")];
    }
    expand_output(result, EXPAND_MAX_LINES, output_style())
}
