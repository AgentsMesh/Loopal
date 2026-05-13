use ratatui::prelude::*;

use loopal_tool_api::TimeoutSecs;
use loopal_view_state::ToolInvocation;

use super::{EXPAND_MAX_LINES, dim_style, expand_output, output_first_line, output_style};

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

pub fn render_running_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let dim = output_style();
    let elapsed_secs = tc.elapsed(std::time::Instant::now()).as_secs_f64();
    let elapsed = format!("{elapsed_secs:.1}s");
    let timeout = tc
        .input
        .as_ref()
        .map(|i| TimeoutSecs::from_tool_input(i, 300))
        .unwrap_or(TimeoutSecs::new(300));

    let mut lines = Vec::new();

    if let Some(tail) = tc.state.progress_tail() {
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
                dim_style(),
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

pub fn render_success_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let Some(content) = tc.state.outcome().map(|o| o.content()) else {
        return vec![output_first_line("(No output)")];
    };
    if content.trim().is_empty() {
        return vec![output_first_line("(No output)")];
    }
    expand_output(content, EXPAND_MAX_LINES, output_style())
}
