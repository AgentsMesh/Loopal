use std::time::Duration;

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use loopal_session::state::SessionState;
use loopal_view_state::AgentConversation;

use super::unified_status_goal::append_goal_indicator;
use super::unified_status_label::{ActivityInputs, pick_label};
use crate::animation::spinner_frame;
use crate::app::App;

// Bridge the brief gap between `AwaitingInput` and the next `Running`
// event (~hub IPC jitter) so the spinner doesn't flicker.
const ACTIVITY_GRACE: Duration = Duration::from_millis(750);

pub fn render_unified_status(
    f: &mut Frame,
    app: &App,
    state: &SessionState,
    conv: &AgentConversation,
    animation_elapsed: Duration,
    area: Rect,
) {
    let observable = app.observable_for(&state.active_view);
    let display_mode = observable.mode.as_str();
    let is_plan = display_mode == "plan";
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(16);
    let base_elapsed = conv.turn_elapsed();
    let is_active = is_agent_active(app, state, conv);

    spans.push(Span::raw(" "));
    let (icon, icon_style, label) =
        status_icon_and_label(app, state, conv, animation_elapsed, is_active);
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

    if let Some(since_ms) = app.hub_degraded_since_for(&state.active_view) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("⚠ Hub degraded {}", format_degraded_age(since_ms)),
            Style::default().fg(Color::Yellow).bold(),
        ));
    }

    let bg = if is_plan {
        Style::default().bg(Color::Rgb(50, 20, 50))
    } else {
        Style::default().bg(Color::Rgb(30, 30, 30))
    };
    f.render_widget(Paragraph::new(Line::from(spans)).style(bg), area);
}

fn status_icon_and_label(
    app: &App,
    state: &SessionState,
    conv: &AgentConversation,
    animation_elapsed: std::time::Duration,
    is_active: bool,
) -> (String, Style, &'static str) {
    let inputs = ActivityInputs {
        thinking: conv.thinking_active,
        compacting: conv.compact_banner.is_some(),
        streaming: !conv.streaming_text.is_empty(),
        pending_permission: conv.pending_permission.is_some()
            || conv.pending_plan_approval.is_some(),
        agent_idle: active_agent_idle(app, state),
        has_subagents: has_live_subagents(app),
        recently_or_active: is_active,
    };
    let (use_spinner, color, label) = pick_label(&inputs);
    let icon = if use_spinner {
        spinner_frame(animation_elapsed).to_string()
    } else {
        "●".to_string()
    };
    (icon, Style::default().fg(color), label)
}

fn is_agent_active(app: &App, state: &SessionState, conv: &AgentConversation) -> bool {
    !active_agent_idle(app, state)
        || !conv.streaming_text.is_empty()
        || conv.thinking_active
        || conv.compact_banner.is_some()
        || has_live_subagents(app)
        || conv.is_recently_active(ACTIVITY_GRACE)
}

fn active_agent_idle(app: &App, state: &SessionState) -> bool {
    use loopal_protocol::AgentStatus;
    let status = app.observable_for(&state.active_view).status;
    matches!(
        status,
        AgentStatus::WaitingForInput | AgentStatus::Finished | AgentStatus::Error
    )
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

pub(super) fn dim_style() -> Style {
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

fn format_degraded_age(since_unix_ms: u64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(since_unix_ms);
    let age_secs = now_ms.saturating_sub(since_unix_ms) / 1000;
    format_duration(Duration::from_secs(age_secs))
}
