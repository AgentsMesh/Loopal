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

pub(crate) use output_format::{
    cancelled_line, completion_line, dim_style, expand_output, output_first_line, output_style,
    stale_line,
};

use ratatui::prelude::*;

use loopal_view_state::{InvocationState, ToolInvocation};

use crate::views::unified_status::spinner_frame;

const EXPAND_MAX_LINES: usize = 4;

pub fn render_tool_calls(tool_calls: &[ToolInvocation], _width: u16) -> Vec<Line<'static>> {
    tool_calls.iter().flat_map(render_one).collect()
}

fn render_one(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let mut lines = vec![render_header(tc)];
    lines.extend(render_body(tc));
    lines
}

fn render_header(tc: &ToolInvocation) -> Line<'static> {
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

fn extract_detail(tc: &ToolInvocation) -> String {
    let Some(ref input) = tc.input else {
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
        "web_search" => input
            .get("query")
            .and_then(|v| v.as_str())
            .map(|s| format!("\"{s}\"")),
        "Agent" => agent::extract_detail(input),
        _ => None,
    };
    truncate_chars(&shorten_home(&raw.unwrap_or_default()), 80)
}

fn render_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    match &tc.state {
        InvocationState::Pending | InvocationState::Running { .. } => match tc.name.as_str() {
            "Bash" => bash::render_running_body(tc),
            "Agent" => agent::render_running_body(tc),
            _ => Vec::new(),
        },
        InvocationState::Done {
            outcome: loopal_view_state::Outcome::Failure { error, .. },
            ..
        } => {
            let mut lines = match tc.state.duration().filter(|d| !d.is_zero()) {
                Some(d) => vec![completion_line("Failed", d)],
                None => Vec::new(),
            };
            lines.extend(expand_output(
                error,
                EXPAND_MAX_LINES,
                Style::default().fg(Color::Red),
            ));
            lines
        }
        InvocationState::Done {
            outcome: loopal_view_state::Outcome::Success { .. },
            ..
        } => {
            let mut lines = match tc.name.as_str() {
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
            };
            if let Some(d) = tc.state.duration().filter(|d| !d.is_zero()) {
                lines.push(Line::from(Span::styled(
                    format!("    Done in {}", output_format::format_duration_short(d)),
                    dim_style(),
                )));
            }
            lines
        }
        InvocationState::Stale { reason, .. } => {
            vec![stale_line(
                &reason.to_string(),
                tc.state.duration().unwrap_or_default(),
            )]
        }
        InvocationState::Cancelled { cause, .. } => {
            vec![cancelled_line(
                &cause.to_string(),
                tc.state.duration().unwrap_or_default(),
            )]
        }
    }
}

fn render_default_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let Some(content) = tc.state.outcome().map(|o| o.content()) else {
        return Vec::new();
    };
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if content.lines().count() <= 1 && trimmed.len() <= 60 {
        return vec![output_first_line(trimmed)];
    }
    expand_output(content, EXPAND_MAX_LINES, output_style())
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

fn status_icon(tc: &ToolInvocation) -> (String, Color) {
    match &tc.state {
        InvocationState::Done {
            outcome: loopal_view_state::Outcome::Success { .. },
            ..
        } => ("●".to_string(), Color::Green),
        InvocationState::Done {
            outcome: loopal_view_state::Outcome::Failure { .. },
            ..
        } => ("●".to_string(), Color::Red),
        InvocationState::Stale { .. } => ("◐".to_string(), Color::Yellow),
        InvocationState::Cancelled { .. } => ("○".to_string(), Color::DarkGray),
        InvocationState::Pending | InvocationState::Running { .. } => {
            let elapsed = tc.elapsed(std::time::Instant::now());
            (spinner_frame(elapsed).to_string(), Color::Yellow)
        }
    }
}
