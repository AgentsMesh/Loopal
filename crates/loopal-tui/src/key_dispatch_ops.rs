//! Side-effect implementations for key dispatch actions.

use loopal_protocol::UserContent;

use crate::app::{App, FocusMode, PanelKind};
use crate::command::CommandEffect;
use crate::input::SubPageResult;
use crate::panel_ops;
use crate::views::bg_tasks_panel;

// Re-export panel operations used by key_dispatch and lib.rs dispatch_ops.
pub use crate::panel_ops::{cycle_panel_focus, enter_panel, panel_tab};

pub(crate) async fn push_to_inbox(app: &mut App, content: UserContent) {
    // For skill invocations, record the slash command (not the expanded body)
    let history_text = match &content.skill_info {
        Some(si) if si.user_args.is_empty() => si.name.clone(),
        Some(si) => format!("{} {}", si.name, si.user_args),
        None => content.text.clone(),
    };
    app.input_history.push(history_text);
    app.history_index = None;
    if let Some(msg) = app.session.enqueue_message(content) {
        tracing::debug!("TUI: message forwarded to agent");
        app.session.route_message(msg).await;
    } else {
        tracing::debug!("TUI: agent busy, message queued + interrupt sent");
        app.session.interrupt();
    }
}

pub(crate) async fn handle_effect(app: &mut App, effect: CommandEffect) -> bool {
    match effect {
        CommandEffect::Done => false,
        CommandEffect::InboxPush(content) => {
            push_to_inbox(app, content).await;
            false
        }
        CommandEffect::ModeSwitch(mode) => {
            app.session.switch_mode(mode).await;
            false
        }
        CommandEffect::Quit => {
            app.exiting = true;
            true
        }
        CommandEffect::ResumeSession(session_id) => {
            app.session.resume_session(&session_id).await;
            false
        }
    }
}

pub(crate) async fn handle_sub_page_confirm(app: &mut App, result: SubPageResult) {
    match result {
        SubPageResult::ModelSelected(name) => {
            app.session.switch_model(name).await;
        }
        SubPageResult::ModelAndThinkingSelected {
            model,
            thinking_json,
        } => {
            app.session.switch_model(model).await;
            app.session.switch_thinking(thinking_json).await;
        }
        SubPageResult::RewindConfirmed(turn_index) => {
            app.session.rewind(turn_index).await;
        }
    }
}

/// Terminate (interrupt) the currently focused agent via Hub.
pub(crate) async fn terminate_focused_agent(app: &mut App) {
    let Some(name) = app.focused_agent.clone() else {
        return;
    };
    if name == loopal_session::ROOT_AGENT {
        return;
    }
    let state = app.session.lock();
    let active = state.active_view.clone();
    drop(state);
    app.session.interrupt_agent(&name);
    if active == name {
        app.session.exit_agent_view();
        app.scroll_offset = 0;
        app.line_cache = crate::views::progress::LineCache::new();
    }
    app.focused_agent = None;
    // If no panels have content, exit panel mode
    if !panel_ops::has_live_agents(app) && bg_tasks_panel::bg_panel_height(&app.bg_snapshots) == 0 {
        app.focus_mode = FocusMode::Input;
        app.agent_panel_offset = 0;
    } else if !panel_ops::has_live_agents(app) {
        app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);
        panel_ops::enter_panel(app);
    }
}
