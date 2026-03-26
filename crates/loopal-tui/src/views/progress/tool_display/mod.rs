//! Tool rendering — each tool independently displayed with expanded output,
//! folded after N lines.
//!
//! ```text
//! ● Bash(git log --oneline -5)
//!   ⎿ fd661d3 feat(tui): progressive disclosure
//!     dc0a5e7 Add fragment-based prompt engine
//!     … +3 lines
//! ```
mod bash;
mod edit;
mod glob;
mod grep;
mod read;
mod write;

use ratatui::prelude::*;

use loopal_session::types::{DisplayToolCall, ToolCallStatus};

/// Max output lines before folding.
const EXPAND_MAX_LINES: usize = 4;

// ── Public entry ──

/// Render all tool calls — each independently, no grouping.
pub fn render_tool_calls(tool_calls: &[DisplayToolCall], _width: u16) -> Vec<Line<'static>> {
    tool_calls.iter().flat_map(render_one).collect()
}

fn render_one(tc: &DisplayToolCall) -> Vec<Line<'static>> {
    let mut lines = vec![render_header(tc)];
    lines.extend(render_body(tc));
    lines
}

// ── Header: ● ToolName(detail) ──

fn render_header(tc: &DisplayToolCall) -> Line<'static> {
    let (icon, color) = status_style(tc.status);
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
fn extract_detail(tc: &DisplayToolCall) -> String {
    let Some(ref input) = tc.tool_input else {
        return String::new();
    };
    let raw = match tc.name.as_str() {
        "Bash" => bash::extract_detail(input),
        "Read" => read::extract_detail(input),
        "Write" => write::extract_detail(input),
        "Edit" | "MultiEdit" => edit::extract_detail(input),
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
        // "WebSearch" = built-in client tool, "web_search" = server-side tool
        "WebSearch" | "web_search" => input
            .get("query")
            .and_then(|v| v.as_str())
            .map(|s| format!("\"{s}\"")),
        _ => None,
    };
    truncate_chars(&shorten_home(&raw.unwrap_or_default()), 80)
}

// ── Body: dispatch per tool type ──

fn render_body(tc: &DisplayToolCall) -> Vec<Line<'static>> {
    // Active (pending/running)
    if tc.status.is_active() {
        return if tc.name == "Bash" {
            bash::render_running_body(tc)
        } else {
            Vec::new()
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
        "Read" => read::render_body(tc),
        "Write" => write::render_body(tc),
        "Edit" | "MultiEdit" => edit::render_body(tc),
        "Grep" => grep::render_body(tc),
        "Glob" => glob::render_body(tc),
        _ => render_default_body(tc),
    }
}

/// Fallback: short inline or expand.
fn render_default_body(tc: &DisplayToolCall) -> Vec<Line<'static>> {
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

// ── Shared helpers (used by sub-modules via `super::`) ──

/// Standard style for tool output text — light enough for dark-mode readability.
pub(crate) fn output_style() -> Style {
    Style::default().fg(Color::Rgb(155, 160, 170))
}

/// Expand output up to `max_lines`, fold the rest.
pub(crate) fn expand_output(content: &str, max_lines: usize, style: Style) -> Vec<Line<'static>> {
    let all: Vec<&str> = content.lines().collect();
    let total = all.len();
    let mut lines = Vec::new();

    for (i, text) in all.iter().take(max_lines).enumerate() {
        let prefix = if i == 0 { "  ⎿ " } else { "    " };
        lines.push(Line::from(Span::styled(format!("{prefix}{text}"), style)));
    }

    if total > max_lines {
        lines.push(Line::from(Span::styled(
            format!("    … +{} lines", total - max_lines),
            Style::default().fg(Color::Rgb(100, 105, 115)),
        )));
    }
    lines
}

/// Single output line with ⎿ prefix.
pub(crate) fn output_first_line(text: &str) -> Line<'static> {
    Line::from(Span::styled(format!("  ⎿ {text}"), output_style()))
}

fn shorten_home(path: &str) -> String {
    for prefix in ["/Users/", "/home/"] {
        if path.starts_with(prefix) {
            if let Some(rest) = path.splitn(4, '/').nth(3) {
                return format!("~/{rest}");
            }
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

fn status_style(status: ToolCallStatus) -> (&'static str, Color) {
    match status {
        ToolCallStatus::Success => ("●", Color::Green),
        ToolCallStatus::Error => ("●", Color::Red),
        _ => ("●", Color::Yellow),
    }
}
