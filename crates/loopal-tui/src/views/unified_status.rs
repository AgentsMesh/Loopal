/// Unified status bar: main agent status + model + context + tokens.
///
/// Animated spinner when agent is active, static icon when idle:
/// `⠹ Streaming  12s  ACT  claude-sonnet  ctx:45k/200k  ↑3.2k ↓1.1k  cache:87%`
///
/// Agent indicators moved to dedicated `agent_panel`.
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::state::SessionState;

/// Braille spinner frames — 10 frames at ~100ms tick = smooth rotation.
pub const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Render the unified status bar (1 line).
pub fn render_unified_status(f: &mut Frame, state: &SessionState, area: Rect) {
    let is_plan = state.mode == "plan";
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(16);
    let elapsed = state.turn_elapsed();
    let is_active = is_agent_active(state);

    // Spinner / status icon + label + elapsed time (primary cluster)
    spans.push(Span::raw(" "));
    let (icon, icon_style, label) = status_icon_and_label(state, elapsed, is_active);
    spans.push(Span::styled(icon, icon_style));
    spans.push(Span::styled(format!(" {label}"), icon_style));
    spans.push(Span::raw("  "));
    let time_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        dim_style()
    };
    spans.push(Span::styled(format_duration(elapsed), time_style));

    // Mode
    spans.push(Span::raw("  "));
    let mode_style = if is_plan {
        Style::default().fg(Color::White).bold()
    } else {
        Style::default().fg(Color::Green).bold()
    };
    spans.push(Span::styled(state.mode.to_uppercase(), mode_style));
    if is_plan {
        spans.push(Span::styled(
            " read-only",
            Style::default().fg(Color::Magenta),
        ));
    }

    // Model
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        state.model.clone(),
        Style::default().fg(Color::Cyan),
    ));

    // Context usage
    spans.push(Span::raw("  "));
    spans.push(Span::styled(context_info(state), dim_style()));

    let bg = if is_plan {
        Style::default().bg(Color::Rgb(50, 20, 50))
    } else {
        Style::default().bg(Color::Rgb(30, 30, 30))
    };
    f.render_widget(Paragraph::new(Line::from(spans)).style(bg), area);
}

/// Determine icon (static or animated), style, and status label.
fn status_icon_and_label(
    state: &SessionState,
    elapsed: std::time::Duration,
    _is_active: bool,
) -> (String, Style, &'static str) {
    if state.thinking_active {
        let frame = spinner_frame(elapsed);
        (
            frame.to_string(),
            Style::default().fg(Color::Magenta),
            "Thinking",
        )
    } else if !state.streaming_text.is_empty() {
        let frame = spinner_frame(elapsed);
        (
            frame.to_string(),
            Style::default().fg(Color::Green),
            "Streaming",
        )
    } else if state.pending_permission.is_some() {
        (
            "●".to_string(),
            Style::default().fg(Color::Yellow),
            "Waiting",
        )
    } else if !state.agent_idle {
        let frame = spinner_frame(elapsed);
        (
            frame.to_string(),
            Style::default().fg(Color::Cyan),
            "Working",
        )
    } else if has_live_subagents(state) {
        let frame = spinner_frame(elapsed);
        (
            frame.to_string(),
            Style::default().fg(Color::Blue),
            "Agents",
        )
    } else {
        (
            "●".to_string(),
            Style::default().fg(Color::DarkGray),
            "Idle",
        )
    }
}

/// Pick a braille spinner frame based on elapsed time.
pub fn spinner_frame(elapsed: std::time::Duration) -> &'static str {
    let idx = (elapsed.as_millis() / 100) as usize % SPINNER.len();
    SPINNER[idx]
}

fn is_agent_active(state: &SessionState) -> bool {
    !state.agent_idle
        || !state.streaming_text.is_empty()
        || state.thinking_active
        || has_live_subagents(state)
}

/// True if any sub-agent is still starting or running.
fn has_live_subagents(state: &SessionState) -> bool {
    use loopal_protocol::AgentStatus;
    state.agents.values().any(|a| {
        matches!(
            a.observable.status,
            AgentStatus::Starting | AgentStatus::Running
        )
    })
}

fn context_info(state: &SessionState) -> String {
    let total = state.token_count();
    if state.context_window > 0 {
        format!("ctx:{}k/{}k", total / 1000, state.context_window / 1000)
    } else {
        format!("{}k tok", total / 1000)
    }
}

fn dim_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Format a Duration as human-readable (e.g., "3m24s", "1h05m").
pub fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}
