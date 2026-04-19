/// Unified status bar: main agent status + model + context + tokens.
///
/// Animated spinner when agent is active, static icon when idle:
/// `⠹ Streaming  12s  ACT  claude-sonnet  ctx:45k/200k  ↑3.2k ↓1.1k  cache:87%`
///
/// Agent indicators moved to dedicated `agent_panel`.
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::state::SessionState;

/// Braille spinner frames — 10 frames at ~100ms tick = smooth rotation.
pub const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Grace window after the last agent-activity event during which we keep the
/// status spinner/timer live. Covers the short gap between `AwaitingInput`
/// and the next `Running` event (events hop across agent-proc → hub → TUI)
/// plus any ordering jitter in the broadcast/mpsc bridge.
const ACTIVITY_GRACE: Duration = Duration::from_millis(750);

/// Render the unified status bar (1 line).
pub fn render_unified_status(f: &mut Frame, state: &SessionState, area: Rect) {
    let is_plan = state.mode == "plan";
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(16);
    let conv = state.active_conversation();
    let base_elapsed = conv.turn_elapsed();
    let is_active = is_agent_active(state);
    // Spinner animation must keep moving even if the per-turn timer froze
    // (e.g. during the brief window between turns or when viewing an agent
    // whose turn just ended). A monotonic global clock decouples the spinner
    // from any specific `turn_start`.
    let spinner_elapsed = if is_active {
        animation_clock()
    } else {
        base_elapsed
    };

    // Spinner / status icon + label + elapsed time (primary cluster)
    spans.push(Span::raw(" "));
    let (icon, icon_style, label) = status_icon_and_label(state, spinner_elapsed, is_active);
    spans.push(Span::styled(icon, icon_style));
    spans.push(Span::styled(format!(" {label}"), icon_style));
    spans.push(Span::raw("  "));
    let time_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        dim_style()
    };
    spans.push(Span::styled(format_duration(base_elapsed), time_style));

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

    // Model — show viewed agent's model if available, fall back to session model
    let display_model = state
        .agents
        .get(&state.active_view)
        .map(|a| a.observable.model.as_str())
        .filter(|m| !m.is_empty())
        .unwrap_or(&state.model);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        display_model.to_string(),
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
    is_active: bool,
) -> (String, Style, &'static str) {
    let conv = state.active_conversation();
    let spin = || spinner_frame(elapsed).to_string();
    if conv.thinking_active {
        (spin(), Style::default().fg(Color::Magenta), "Thinking")
    } else if !conv.streaming_text.is_empty() {
        (spin(), Style::default().fg(Color::Green), "Streaming")
    } else if conv.pending_permission.is_some() {
        ("●".into(), Style::default().fg(Color::Yellow), "Waiting")
    } else if !state.is_active_agent_idle() {
        (spin(), Style::default().fg(Color::Cyan), "Working")
    } else if has_live_subagents(state) {
        (spin(), Style::default().fg(Color::Blue), "Agents")
    } else if is_active {
        // Grace window: the agent's observable status just flipped to
        // WaitingForInput but an activity event landed recently — keep
        // the spinner alive until the authoritative Running event arrives.
        (spin(), Style::default().fg(Color::Cyan), "Working")
    } else {
        ("●".into(), Style::default().fg(Color::DarkGray), "Idle")
    }
}

/// Pick a braille spinner frame based on elapsed time.
pub fn spinner_frame(elapsed: std::time::Duration) -> &'static str {
    let idx = (elapsed.as_millis() / 100) as usize % SPINNER.len();
    SPINNER[idx]
}

fn is_agent_active(state: &SessionState) -> bool {
    let conv = state.active_conversation();
    !state.is_active_agent_idle()
        || !conv.streaming_text.is_empty()
        || conv.thinking_active
        || has_live_subagents(state)
        // Bridge the brief gap between an `AwaitingInput` event and the
        // next `Running` event that re-arms the turn timer. Without this
        // grace, the spinner flickers off between turns.
        || conv.is_recently_active(ACTIVITY_GRACE)
}

/// Process-wide monotonic clock for spinner animation. Decouples the
/// spinner's frame progression from any specific turn timer so that even
/// if `turn_elapsed` momentarily freezes (e.g. between `AwaitingInput` and
/// `Running`), the spinner keeps rotating smoothly while the agent is
/// otherwise active.
fn animation_clock() -> Duration {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
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
    let conv = state.active_conversation();
    let total = conv.token_count();
    if conv.context_window > 0 {
        format!("ctx:{}k/{}k", total / 1000, conv.context_window / 1000)
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
