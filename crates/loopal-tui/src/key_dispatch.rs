//! Key-action dispatch — maps InputAction → side effects + quit flag.

use std::sync::Arc;

use loopal_agent::router::MessageRouter;
use loopal_protocol::AgentMode;

use crate::app::App;
use crate::event::EventHandler;
use crate::input::paste;
use crate::input::{InputAction, handle_key};
use crate::slash_handler::handle_slash_command;
use crate::tui_helpers::{cycle_focus, handle_question_confirm, route_human_message};

/// Process a single key event and return `true` if the TUI should quit.
pub(crate) async fn handle_key_action(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    router: &Arc<MessageRouter>,
    target_agent: &str,
    events: &EventHandler,
) -> bool {
    let action = handle_key(app, key);
    match action {
        InputAction::Quit => {
            app.exiting = true;
            true
        }
        InputAction::InboxPush(content) => {
            app.input_history.push(content.text.clone());
            app.history_index = None;
            if let Some(msg) = app.session.enqueue_message(content) {
                route_human_message(router, target_agent, msg).await;
            } else {
                app.session.interrupt();
            }
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
        InputAction::SlashCommand(cmd) => {
            handle_slash_command(app, cmd).await;
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
