/// Task summary bar: status icon + turn duration + token statistics (1 line).
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::state::SessionState;

/// Render the task summary bar (1 line).
pub fn render_task_summary(f: &mut Frame, state: &SessionState, area: Rect) {
    let status = if !state.streaming_text.is_empty() {
        Span::styled("● Streaming", Style::default().fg(Color::Green))
    } else if state.pending_permission.is_some() {
        Span::styled("● Waiting", Style::default().fg(Color::Yellow))
    } else if state.agent_idle {
        Span::styled("● Idle", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled("● Working", Style::default().fg(Color::Cyan))
    };

    let elapsed = state.turn_elapsed();
    let duration = format_duration(elapsed);

    let token_count = state.token_count();
    let token_info = if state.context_window > 0 {
        format!(
            "↑{}k ↓{}k",
            state.input_tokens / 1000,
            state.output_tokens / 1000
        )
    } else {
        format!("{} tokens", token_count)
    };

    let spans = vec![
        Span::raw(" "),
        status,
        Span::raw("  "),
        Span::styled(duration, Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(token_info, Style::default().fg(Color::DarkGray)),
    ];

    let paragraph = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::Rgb(30, 30, 30)));

    f.render_widget(paragraph, area);
}

/// Format a Duration as human-readable (e.g., "3m24s", "1h05m").
fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}
