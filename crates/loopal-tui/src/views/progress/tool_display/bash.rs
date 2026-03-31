//! Bash tool rendering: header detail + body (running / success).

use ratatui::prelude::*;

use loopal_session::types::SessionToolCall;
use loopal_tool_api::TimeoutSecs;

use super::{EXPAND_MAX_LINES, expand_output, output_first_line, output_style};

/// Extract Bash command for header: strip `cd ... &&` preamble, collapse whitespace.
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    let cmd = input.get("command").and_then(|v| v.as_str())?;
    let cleaned = if let Some(pos) = cmd.find("&&") {
        let before = cmd[..pos].trim();
        if before.starts_with("cd ") {
            cmd[pos + 2..].trim()
        } else {
            cmd
        }
    } else {
        cmd
    };
    Some(cleaned.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Running Bash: elapsed time + progress tail.
pub fn render_running_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let dim = output_style();
    let elapsed = tc
        .started_at
        .map(|t| format!("{:.1}s", t.elapsed().as_secs_f64()))
        .unwrap_or_else(|| "…".to_string());
    let timeout = tc
        .tool_input
        .as_ref()
        .map(|i| TimeoutSecs::from_tool_input(i, 300))
        .unwrap_or(TimeoutSecs::new(300));

    let mut lines = Vec::new();

    if let Some(ref tail) = tc.progress_tail {
        let tail_trimmed = tail.trim();
        if !tail_trimmed.is_empty() {
            let tail_lines: Vec<&str> = tail_trimmed.lines().collect();
            let show = &tail_lines[tail_lines.len().saturating_sub(2)..];
            if let Some(first) = show.first() {
                lines.push(Line::from(Span::styled(format!("  ⎿ {first}"), dim)));
            }
            for tl in show.iter().skip(1) {
                lines.push(Line::from(Span::styled(format!("    {tl}"), dim)));
            }
            lines.push(Line::from(Span::styled(
                format!("    ({elapsed} / {timeout})"),
                Style::default().fg(Color::Rgb(100, 105, 115)),
            )));
            return lines;
        }
    }

    lines.push(Line::from(Span::styled(
        format!("  ⎿ Running… ({elapsed} / {timeout})"),
        dim,
    )));
    lines
}

/// Completed Bash: expand stdout.
pub fn render_success_body(tc: &SessionToolCall) -> Vec<Line<'static>> {
    let Some(ref result) = tc.result else {
        return vec![output_first_line("(No output)")];
    };
    if result.trim().is_empty() {
        return vec![output_first_line("(No output)")];
    }
    expand_output(result, EXPAND_MAX_LINES, output_style())
}
