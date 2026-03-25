use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use ratatui::prelude::*;

use loopal_agent::router::MessageRouter;
use loopal_protocol::AgentEvent;
use loopal_protocol::AgentMode;
use loopal_session::SessionController;
use tokio::sync::mpsc;

use crate::app::App;
use crate::command::CommandEntry;
use crate::event::{AppEvent, EventHandler};
use crate::input::paste;
use crate::input::{InputAction, handle_key};
use crate::render::draw;
use crate::slash_handler::handle_slash_command;
use crate::terminal::TerminalGuard;
use crate::tui_helpers::{
    cycle_focus, handle_question_confirm, route_human_message,
};

/// Run the TUI event loop.
///
/// The TUI owns the `router` for sending user messages (data plane)
/// and `session` for observation and control (observation + control planes).
pub async fn run_tui(
    session: SessionController,
    router: Arc<MessageRouter>,
    target_agent: String,
    commands: Vec<CommandEntry>,
    cwd: PathBuf,
    agent_event_rx: mpsc::Receiver<AgentEvent>,
) -> anyhow::Result<()> {
    let _guard = TerminalGuard::new()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(session, commands, cwd);
    let mut events = EventHandler::new(agent_event_rx);

    terminal.draw(|f| draw(f, &mut app))?;

    loop {
        let Some(first) = events.next().await else {
            break;
        };

        let mut batch = vec![first];
        while let Some(event) = events.try_next() {
            batch.push(event);
        }

        let mut should_quit = false;
        for event in batch {
            match event {
                AppEvent::Key(key) => {
                    let action = handle_key(&mut app, key);
                    if matches!(action, InputAction::PasteRequested) {
                        paste::spawn_paste(&events);
                    } else {
                        should_quit =
                            dispatch_action(&mut app, &router, &target_agent, action).await;
                    }
                    if should_quit {
                        break;
                    }
                }
                AppEvent::Agent(agent_event) => {
                    if let Some(content) = app.session.handle_event(agent_event) {
                        route_human_message(&router, &target_agent, content).await;
                    }
                }
                AppEvent::Paste(result) => {
                    paste::apply_paste_result(&mut app, result);
                }
                AppEvent::Resize(_, _) => {}
                AppEvent::Tick => {}
            }
        }

        if should_quit || app.exiting {
            break;
        }
        terminal.draw(|f| draw(f, &mut app))?;
    }

    terminal.show_cursor()?;
    Ok(())
}

/// Dispatch an InputAction. Returns true if the app should quit.
async fn dispatch_action(
    app: &mut App,
    router: &Arc<MessageRouter>,
    target_agent: &str,
    action: InputAction,
) -> bool {
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
        InputAction::PasteRequested => false, // handled separately via events.sender()
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
