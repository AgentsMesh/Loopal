use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_protocol::{ThreadGoal, ThreadGoalStatus};
use loopal_session::state::SessionState;
use loopal_view_state::AgentConversation;

use crate::app::App;

pub const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Bridge the brief gap between `AwaitingInput` and the next `Running`
/// event (~hub IPC jitter) so the spinner doesn't flicker.
const ACTIVITY_GRACE: Duration = Duration::from_millis(750);

pub fn render_unified_status(
    f: &mut Frame,
    app: &App,
    state: &SessionState,
    conv: &AgentConversation,
    area: Rect,
) {
    let observable = app.observable_for(&state.active_view);
    let display_mode = observable.mode.as_str();
    let is_plan = display_mode == "plan";
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(16);
    let base_elapsed = conv.turn_elapsed();
    let is_active = is_agent_active(app, state, conv);
    let spinner_elapsed = if is_active {
        animation_clock()
    } else {
        base_elapsed
    };

    spans.push(Span::raw(" "));
    let (icon, icon_style, label) =
        status_icon_and_label(app, state, conv, spinner_elapsed, is_active);
    spans.push(Span::styled(icon, icon_style));
    spans.push(Span::styled(format!(" {label}"), icon_style));
    spans.push(Span::raw("  "));
    let time_style = if is_active {
        Style::default().fg(Color::White)
    } else {
        dim_style()
    };
    spans.push(Span::styled(format_duration(base_elapsed), time_style));

    spans.push(Span::raw("  "));
    let mode_style = if is_plan {
        Style::default().fg(Color::White).bold()
    } else {
        Style::default().fg(Color::Green).bold()
    };
    spans.push(Span::styled(display_mode.to_uppercase(), mode_style));
    if is_plan {
        spans.push(Span::styled(
            " read-only",
            Style::default().fg(Color::Magenta),
        ));
    }

    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        observable.model.clone(),
        Style::default().fg(Color::Cyan),
    ));

    spans.push(Span::raw("  "));
    spans.push(Span::styled(context_info(conv), dim_style()));

    if let Some(goal) = app.thread_goal_for(&state.active_view) {
        spans.push(Span::raw("  "));
        append_goal_indicator(&mut spans, &goal);
    }

    let bg = if is_plan {
        Style::default().bg(Color::Rgb(50, 20, 50))
    } else {
        Style::default().bg(Color::Rgb(30, 30, 30))
    };
    f.render_widget(Paragraph::new(Line::from(spans)).style(bg), area);
}

fn append_goal_indicator(spans: &mut Vec<Span<'static>>, goal: &ThreadGoal) {
    let (label, color) = match goal.status {
        ThreadGoalStatus::Active => ("active", Color::Cyan),
        ThreadGoalStatus::Paused => ("paused", Color::Yellow),
        ThreadGoalStatus::BudgetLimited => ("budget", Color::Red),
        ThreadGoalStatus::Complete => ("done", Color::Green),
    };
    spans.push(Span::styled("◆ ", Style::default().fg(color).bold()));
    spans.push(Span::styled(
        truncate_objective(&goal.objective),
        Style::default().fg(color),
    ));
    spans.push(Span::styled(
        format!(" [{label}]"),
        Style::default().fg(color).bold(),
    ));
    if let Some(b) = goal.token_budget {
        let used_k = goal.tokens_used / 1000;
        let budget_k = b / 1000;
        let usage_color = if goal.budget_exhausted() {
            Color::Red
        } else {
            Color::DarkGray
        };
        spans.push(Span::styled(
            format!(" {used_k}k/{budget_k}k"),
            Style::default().fg(usage_color),
        ));
    } else if goal.tokens_used > 0 {
        spans.push(Span::styled(
            format!(" {}k", goal.tokens_used / 1000),
            dim_style(),
        ));
    }
}

fn truncate_objective(s: &str) -> String {
    const MAX: usize = 28;
    let trimmed: String = s.chars().take(MAX).collect();
    if s.chars().count() > MAX {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

fn status_icon_and_label(
    app: &App,
    state: &SessionState,
    conv: &AgentConversation,
    elapsed: std::time::Duration,
    is_active: bool,
) -> (String, Style, &'static str) {
    let spin = || spinner_frame(elapsed).to_string();
    if conv.thinking_active {
        (spin(), Style::default().fg(Color::Magenta), "Thinking")
    } else if !conv.streaming_text.is_empty() {
        (spin(), Style::default().fg(Color::Green), "Streaming")
    } else if conv.pending_permission.is_some() {
        ("●".into(), Style::default().fg(Color::Yellow), "Waiting")
    } else if !active_agent_idle(app, state) {
        (spin(), Style::default().fg(Color::Cyan), "Working")
    } else if has_live_subagents(app) {
        (spin(), Style::default().fg(Color::Blue), "Agents")
    } else if is_active {
        (spin(), Style::default().fg(Color::Cyan), "Working")
    } else {
        ("●".into(), Style::default().fg(Color::DarkGray), "Idle")
    }
}

pub fn spinner_frame(elapsed: std::time::Duration) -> &'static str {
    let idx = (elapsed.as_millis() / 100) as usize % SPINNER.len();
    SPINNER[idx]
}

fn is_agent_active(app: &App, state: &SessionState, conv: &AgentConversation) -> bool {
    !active_agent_idle(app, state)
        || !conv.streaming_text.is_empty()
        || conv.thinking_active
        || has_live_subagents(app)
        || conv.is_recently_active(ACTIVITY_GRACE)
}

/// Idle check that does not re-acquire the session lock — `state` is
/// already held by the caller, we just look up the active ViewClient.
fn active_agent_idle(app: &App, state: &SessionState) -> bool {
    use loopal_protocol::AgentStatus;
    let status = app.observable_for(&state.active_view).status;
    matches!(
        status,
        AgentStatus::WaitingForInput | AgentStatus::Finished | AgentStatus::Error
    )
}

fn animation_clock() -> Duration {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

fn has_live_subagents(app: &App) -> bool {
    use loopal_protocol::AgentStatus;
    app.view_clients.iter().any(|(name, vc)| {
        if name == "main" {
            return false;
        }
        let status = vc.state().state().agent.observable.status;
        matches!(status, AgentStatus::Starting | AgentStatus::Running)
    })
}

fn context_info(conv: &AgentConversation) -> String {
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
