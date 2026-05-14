use ratatui::prelude::*;

use loopal_view_state::ToolInvocation;

use super::{EXPAND_MAX_LINES, dim_style, expand_output, output_first_line, output_style};

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

pub fn render_running_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let dim = output_style();
    let elapsed_secs = tc.elapsed(std::time::Instant::now()).as_secs_f64();
    let elapsed = format!("{elapsed_secs:.1}s");

    if let Some(tail) = tc.state.progress_tail() {
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

pub fn render_success_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let Some(content) = tc.state.outcome().map(|o| o.content()) else {
        return vec![output_first_line("(No output)")];
    };
    if content.trim().is_empty() {
        return vec![output_first_line("(No output)")];
    }
    expand_output(content, EXPAND_MAX_LINES, output_style())
}
