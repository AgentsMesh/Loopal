mod agent;
mod apply_patch;
mod bash;
mod diff_style;
mod edit;
mod glob;
mod grep;
mod output_format;
mod read;
mod write;

pub(crate) use output_format::{dim_style, expand_output, output_first_line, output_style};

use ratatui::prelude::*;

use loopal_view_state::{SessionToolCall, ToolCallStatus};

use crate::views::unified_status::spinner_frame;

/// Max output lines before folding.
const EXPAND_MAX_LINES: usize = 4;

// ── Public entry ──

/// Render all tool calls — each independently, no grouping.
pub fn render_tool_calls(tool_calls: &[SessionToolCall], _width: u16) -> Vec<Line<'static>> {
    tool_calls.iter().flat_map(render_one).collect()
}

fn render_one(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let mut lines = vec![render_header(tc)];
    lines.extend(render_body(tc));
    lines
}

// ── Header: ● ToolName(detail) ──

fn render_header(tc: &SessionToolCall) -> Line<'static> {
    let (icon, color) = status_icon(tc);
    let detail = extract_detail(tc);

    let mut spans = vec![
        Span::styled(format!("{icon} "), Style::default().fg(color)),
        Span::styled(tc.name.clone(), Style::default().fg(color).bold()),
    ];
    if !detail.is_empty() {
        spans.push(Span::styled(
            format!("({detail})"),
            Style::default().fg(Color::Rgb(130, 135, 145)),
        ));
    }
    Line::from(spans)
}

/// Dispatch detail extraction to per-tool modules.
fn extract_detail(tc: &SessionToolCall) -> String {
    let Some(ref input) = tc.tool_input else {
        return String::new();
    };
    let raw = match tc.name.as_str() {
        "Bash" => bash::extract_detail(input),
        "Read" => read::extract_detail(input),
        "Write" => write::extract_detail(input),
        "Edit" | "MultiEdit" => edit::extract_detail(input),
        "ApplyPatch" => apply_patch::extract_detail(input),
        "Grep" => grep::extract_detail(input),
        "Glob" => glob::extract_detail(input),
        "Ls" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "WebFetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        // "web_search" = server-side search tool provided by LLM provider
        "web_search" => input
            .get("query")
            .and_then(|v| v.as_str())
            .map(|s| format!("\"{s}\"")),
        "Agent" => agent::extract_detail(input),
        _ => None,
    };
    truncate_chars(&shorten_home(&raw.unwrap_or_default()), 80)
}

// ── Body: dispatch per tool type ──

fn render_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    // Active (pending/running)
    if tc.status.is_active() {
        return match tc.name.as_str() {
            "Bash" => bash::render_running_body(tc),
            "Agent" => agent::render_running_body(tc),
            _ => Vec::new(),
        };
    }
    // Error — shared: expand first N error lines
    if tc.status == ToolCallStatus::Error {
        let Some(ref result) = tc.result else {
            return vec![output_first_line("error")];
        };
        return expand_output(result, EXPAND_MAX_LINES, Style::default().fg(Color::Red));
    }
    // Success — per-tool dispatch
    match tc.name.as_str() {
        "Bash" => bash::render_success_body(tc),
        "Agent" => agent::render_success_body(tc),
        "Read" => read::render_body(tc),
        "Write" => write::render_body(tc),
        "Edit" => edit::render_body(tc),
        "MultiEdit" => edit::render_multi_edit_body(tc),
        "ApplyPatch" => apply_patch::render_body(tc),
        "Grep" => grep::render_body(tc),
        "Glob" => glob::render_body(tc),
        _ => render_default_body(tc),
    }
}

/// Fallback: short inline or expand.
fn render_default_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let Some(ref result) = tc.result else {
        return Vec::new();
    };
    let trimmed = result.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if result.lines().count() <= 1 && trimmed.len() <= 60 {
        return vec![output_first_line(trimmed)];
    }
    expand_output(result, EXPAND_MAX_LINES, output_style())
}

fn shorten_home(path: &str) -> String {
    for prefix in ["/Users/", "/home/"] {
        if path.starts_with(prefix)
            && let Some(rest) = path.splitn(4, '/').nth(3)
        {
            return format!("~/{rest}");
        }
    }
    path.to_string()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

fn status_icon(tc: &SessionToolCall) -> (String, Color) {
    match tc.status {
        ToolCallStatus::Success => ("●".to_string(), Color::Green),
        ToolCallStatus::Error => ("●".to_string(), Color::Red),
        _ => {
            let elapsed = tc
                .started_at
                .map_or(std::time::Duration::ZERO, |t| t.elapsed());
            (spinner_frame(elapsed).to_string(), Color::Yellow)
        }
    }
}
