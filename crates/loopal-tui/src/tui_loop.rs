use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use ratatui::prelude::*;

use loopal_agent::router::MessageRouter;
use loopal_session::SessionController;
use loopal_protocol::AgentMode;
use loopal_protocol::{Envelope, MessageSource};
use loopal_protocol::AgentEvent;
use tokio::sync::mpsc;

use crate::app::App;
use crate::command::CommandEntry;
use crate::event::{AppEvent, EventHandler};
use crate::input::{InputAction, handle_key};
use crate::render::draw;
use crate::slash_handler::handle_slash_command;
use crate::terminal::TerminalGuard;

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
        let Some(first) = events.next().await else { break; };

        let mut batch = vec![first];
        while let Some(event) = events.try_next() {
            batch.push(event);
        }

        let mut should_quit = false;
        for event in batch {
            match event {
                AppEvent::Key(key) => {
                    let action = handle_key(&mut app, key);
                    match action {
                        InputAction::Quit => {
                            app.exiting = true;
                            should_quit = true;
                            break;
                        }
                        InputAction::InboxPush(text) => {
                            app.input_history.push(text.clone());
                            app.history_index = None;
                            if let Some(msg) = app.session.enqueue_message(text) {
                                route_human_message(&router, &target_agent, msg).await;
                            }
                        }
                        InputAction::ToolApprove => {
                            let has = app.session.lock().pending_permission.is_some();
                            if has { app.session.approve_permission().await; }
                        }
                        InputAction::ToolDeny => {
                            let has = app.session.lock().pending_permission.is_some();
                            if has { app.session.deny_permission().await; }
                        }
                        InputAction::ModeSwitch(mode) => {
                            let m = if mode == "plan" { AgentMode::Plan } else { AgentMode::Act };
                            app.session.switch_mode(m).await;
                        }
                        InputAction::SlashCommand(cmd) => {
                            handle_slash_command(&mut app, cmd).await;
                        }
                        InputAction::FocusNextAgent => { cycle_focus(&app); }
                        InputAction::UnfocusAgent => {
                            app.session.lock().focused_agent = None;
                        }
                        InputAction::QuestionUp => {
                            if let Some(ref mut q) = app.session.lock().pending_question {
                                q.cursor_up();
                            }
                        }
                        InputAction::QuestionDown => {
                            if let Some(ref mut q) = app.session.lock().pending_question {
                                q.cursor_down();
                            }
                        }
                        InputAction::QuestionToggle => {
                            if let Some(ref mut q) = app.session.lock().pending_question {
                                q.toggle();
                            }
                        }
                        InputAction::QuestionConfirm => {
                            let answers = {
                                let state = app.session.lock();
                                state.pending_question.as_ref().map(|q| {
                                    let answers = q.get_answers();
                                    if answers.is_empty() && !q.questions[q.current_question].allow_multiple {
                                        // Single-select: select cursor item
                                        let opt = &q.questions[q.current_question].options[q.cursor];
                                        vec![opt.label.clone()]
                                    } else {
                                        answers
                                    }
                                })
                            };
                            if let Some(answers) = answers {
                                app.session.answer_question(answers).await;
                            }
                        }
                        InputAction::QuestionCancel => {
                            app.session.answer_question(vec!["(cancelled)".into()]).await;
                        }
                        InputAction::None => {}
                    }
                }
                AppEvent::Agent(agent_event) => {
                    if let Some(msg) = app.session.handle_event(agent_event) {
                        route_human_message(&router, &target_agent, msg).await;
                    }
                }
                AppEvent::Mouse(mouse) => {
                    use crossterm::event::MouseEventKind;
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            app.scroll_offset = app.scroll_offset.saturating_add(3);
                        }
                        MouseEventKind::ScrollDown => {
                            app.scroll_offset = app.scroll_offset.saturating_sub(3);
                        }
                        _ => {}
                    }
                }
                AppEvent::Resize(_, _) => {}
                AppEvent::Tick => {}
            }
        }

        if should_quit || app.exiting { break; }
        terminal.draw(|f| draw(f, &mut app))?;
    }

    terminal.show_cursor()?;
    Ok(())
}

/// Route a human message through the data plane.
async fn route_human_message(router: &MessageRouter, target: &str, text: String) {
    let envelope = Envelope::new(MessageSource::Human, target, text);
    if let Err(e) = router.route(envelope).await {
        tracing::warn!(error = %e, "failed to route human message — agent may have exited");
    }
}

/// Cycle focused_agent to the next agent in the agents map.
fn cycle_focus(app: &App) {
    let mut state = app.session.lock();
    let keys: Vec<String> = state.agents.keys().cloned().collect();
    if keys.is_empty() {
        state.focused_agent = None;
        return;
    }
    let next = match &state.focused_agent {
        None => keys[0].clone(),
        Some(current) => {
            let pos = keys.iter().position(|k| k == current);
            match pos {
                Some(i) if i + 1 < keys.len() => keys[i + 1].clone(),
                _ => keys[0].clone(),
            }
        }
    };
    state.focused_agent = Some(next);
}
