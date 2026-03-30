//! Key-action dispatch — maps InputAction → side effects + quit flag.

use loopal_protocol::{AgentMode, UserContent};

use crate::app::App;
use crate::command::CommandEffect;
use crate::event::EventHandler;
use crate::input::paste;
use crate::input::{InputAction, SubPageResult, handle_key};
use crate::tui_helpers::{cycle_focus, handle_question_confirm, route_human_message};

/// Process a single key event and return `true` if the TUI should quit.
pub(crate) async fn handle_key_action(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    events: &EventHandler,
) -> bool {
    let action = handle_key(app, key);
    match action {
        InputAction::Quit => {
            app.exiting = true;
            true
        }
        InputAction::InboxPush(content) => {
            push_to_inbox(app, content).await;
            false
        }
        InputAction::PasteRequested => {
            paste::spawn_paste(events);
            false
        }
        InputAction::ToolApprove => {
            if app.session.lock().pending_permission.is_some() {
                app.session.approve_permission().await;
            }
            false
        }
        InputAction::ToolDeny => {
            if app.session.lock().pending_permission.is_some() {
                app.session.deny_permission().await;
            }
            false
        }
        InputAction::Interrupt => {
            app.session.interrupt();
            false
        }
        InputAction::ModeSwitch(mode) => {
            let m = if mode == "plan" {
                AgentMode::Plan
            } else {
                AgentMode::Act
            };
            app.session.switch_mode(m).await;
            false
        }
        InputAction::RunCommand(name, arg) => {
            if let Some(handler) = app.command_registry.find(&name) {
                let effect = handler.execute(app, arg.as_deref()).await;
                handle_effect(app, effect).await
            } else {
                false
            }
        }
        InputAction::SubPageConfirm(result) => {
            handle_sub_page_confirm(app, result).await;
            false
        }
        InputAction::FocusNextAgent => {
            cycle_focus(app);
            false
        }
        InputAction::UnfocusAgent => {
            app.session.lock().focused_agent = None;
            false
        }
        InputAction::QuestionUp => {
            if let Some(ref mut q) = app.session.lock().pending_question {
                q.cursor_up();
            }
            false
        }
        InputAction::QuestionDown => {
            if let Some(ref mut q) = app.session.lock().pending_question {
                q.cursor_down();
            }
            false
        }
        InputAction::QuestionToggle => {
            if let Some(ref mut q) = app.session.lock().pending_question {
                q.toggle();
            }
            false
        }
        InputAction::QuestionConfirm => {
            handle_question_confirm(app).await;
            false
        }
        InputAction::QuestionCancel => {
            app.session
                .answer_question(vec!["(cancelled)".into()])
                .await;
            false
        }
        InputAction::None => false,
    }
}

async fn push_to_inbox(app: &mut App, content: UserContent) {
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
        route_human_message(app, msg).await;
    } else {
        tracing::debug!("TUI: agent busy, message queued + interrupt sent");
        app.session.interrupt();
    }
}

async fn handle_effect(app: &mut App, effect: CommandEffect) -> bool {
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

async fn handle_sub_page_confirm(app: &mut App, result: SubPageResult) {
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
