//! Bash tool rendering: header detail + body (running / success).

use ratatui::prelude::*;

use loopal_session::types::DisplayToolCall;

use super::{expand_output, output_first_line, EXPAND_MAX_LINES};

/// Extract Bash command for header: strip `cd ... &&` preamble, collapse whitespace.
pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    let cmd = input.get("command").and_then(|v| v.as_str())?;
    let cleaned = if let Some(pos) = cmd.find("&&") {
        let before = cmd[..pos].trim();
        if before.starts_with("cd ") { cmd[pos + 2..].trim() } else { cmd }
    } else {
        cmd
    };
    Some(cleaned.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Running Bash: elapsed time + progress tail.
pub fn render_running_body(tc: &DisplayToolCall) -> Vec<Line<'static>> {
    let dim = Style::default().fg(Color::DarkGray);
    let elapsed = tc
        .started_at
        .map(|t| format!("{:.1}s", t.elapsed().as_secs_f64()))
        .unwrap_or_else(|| "…".to_string());
    let timeout_ms = tc
        .tool_input
        .as_ref()
        .and_then(|i| i["timeout"].as_u64())
        .unwrap_or(300_000);
    let timeout = format!("{:.0}s", timeout_ms as f64 / 1000.0);

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
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
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
pub fn render_success_body(tc: &DisplayToolCall) -> Vec<Line<'static>> {
    let Some(ref result) = tc.result else { return vec![output_first_line("(No output)")] };
    if result.trim().is_empty() {
        return vec![output_first_line("(No output)")];
    }
    expand_output(result, EXPAND_MAX_LINES, Style::default().fg(Color::DarkGray))
}
