//! Side-effect implementations for key dispatch actions.

use loopal_protocol::UserContent;

use loopal_protocol::AgentStatus;

use crate::app::App;
use crate::command::CommandEffect;
use crate::input::SubPageResult;

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

/// Cycle `focused_agent` in the panel. `forward=true` → next, `false` → prev.
/// Skips the active_view agent (it's the current conversation).
pub(crate) fn cycle_agent_focus(app: &mut App, forward: bool) {
    let state = app.session.lock();
    let active = &state.active_view;
    let keys: Vec<String> = state
        .agents
        .iter()
        .filter(|(k, a)| k.as_str() != active && is_agent_live(&a.observable.status))
        .map(|(k, _)| k.clone())
        .collect();
    drop(state);
    if keys.is_empty() {
        app.focused_agent = None;
        return;
    }
    app.focused_agent = Some(match &app.focused_agent {
        None => {
            if forward {
                keys[0].clone()
            } else {
                keys[keys.len() - 1].clone()
            }
        }
        Some(current) => {
            let pos = keys.iter().position(|k| k == current);
            match pos {
                Some(i) => {
                    if forward {
                        keys[(i + 1) % keys.len()].clone()
                    } else {
                        keys[(i + keys.len() - 1) % keys.len()].clone()
                    }
                }
                None => keys[0].clone(),
            }
        }
    });
}

/// Terminate (interrupt) the currently focused agent via Hub.
/// Refuses to terminate the root agent — that would be catastrophic.
pub(crate) async fn terminate_focused_agent(app: &mut App) {
    let Some(name) = app.focused_agent.clone() else {
        return;
    };
    if name == loopal_session::ROOT_AGENT {
        return;
    }
    // Interrupt the agent — Hub will cascade to children
    let state = app.session.lock();
    let active = state.active_view.clone();
    drop(state);
    app.session.interrupt_agent(&name);
    // If we're viewing the terminated agent, return to root
    if active == name {
        app.session.exit_agent_view();
        app.scroll_offset = 0;
        app.line_cache = crate::views::progress::LineCache::new();
    }
    app.focused_agent = None;
}

fn is_agent_live(status: &AgentStatus) -> bool {
    !matches!(status, AgentStatus::Finished | AgentStatus::Error)
}
