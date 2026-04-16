//! Side-effect implementations for key dispatch actions.

use loopal_protocol::{ControlCommand, UserContent};

use crate::app::{App, PanelKind};
use crate::command::CommandEffect;
use crate::input::SubPageResult;
use crate::panel_ops;

pub use crate::panel_ops::{cycle_panel_focus, enter_panel, panel_tab};

pub(crate) async fn push_to_inbox(app: &mut App, content: UserContent) {
    let history_text = match &content.skill_info {
        Some(si) if si.user_args.is_empty() => si.name.clone(),
        Some(si) => format!("{} {}", si.name, si.user_args),
        None => content.text.clone(),
    };
    app.input_history.push(history_text);
    app.history_index = None;
    app.session.append_user_display(&content);
    app.session.route_message(content).await;
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
        SubPageResult::SessionSelected(session_id) => {
            app.session.resume_session(&session_id).await;
        }
    }
}

pub(crate) async fn mcp_reconnect(app: &mut App, server: String) {
    let target = app.session.lock().active_view.clone();
    app.session
        .send_control(target, ControlCommand::McpReconnect { server })
        .await;
}

/// Terminate (interrupt) the currently focused agent via Hub.
pub(crate) async fn terminate_focused_agent(app: &mut App) {
    let Some(name) = app.section(PanelKind::Agents).focused.clone() else {
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
        app.content_scroll.reset();
    }
    app.section_mut(PanelKind::Agents).focused = None;
    if !panel_ops::has_live_agents(app) {
        panel_ops::enter_panel(app);
        // enter_panel is a no-op if no panels have content → stays Input
    }
}
