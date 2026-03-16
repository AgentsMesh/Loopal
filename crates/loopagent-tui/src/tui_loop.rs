use std::io;

use ratatui::prelude::*;
use tokio::sync::mpsc;

use crate::app::{App, AppState};
use crate::event::{AppEvent, EventHandler};
use crate::input::{InputAction, handle_key};
use crate::render::draw;
use crate::slash_handler::handle_slash_command;
use crate::terminal::TerminalGuard;
use loopagent_types::command::{AgentMode, UserCommand};
use loopagent_types::event::AgentEvent;

/// Run the TUI event loop.
///
/// * `model` - model name to display
/// * `mode` - initial mode ("act" or "plan")
/// * `agent_event_rx` - receives events from the agent runtime
/// * `user_input_tx` - sends user input text to the runtime
/// * `permission_tx` - sends tool permission decisions (id, approved) to the runtime
pub async fn run_tui(
    model: String,
    mode: String,
    agent_event_rx: mpsc::Receiver<AgentEvent>,
    user_input_tx: mpsc::Sender<UserCommand>,
    permission_tx: mpsc::Sender<(String, bool)>,
) -> anyhow::Result<()> {
    // Setup terminal — guard ensures cleanup even on panic or early return
    let _guard = TerminalGuard::new()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create a dummy event_tx (the App needs one but we forward via user_input_tx)
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(16);

    let mut app = App::new(model, mode, event_tx);
    let mut events = EventHandler::new(agent_event_rx);

    // Initial draw
    terminal.draw(|f| draw(f, &app))?;

    // Main loop: batch-process all pending events, then draw once.
    // This avoids re-drawing for every single Stream text chunk,
    // which would throttle event consumption and cause back-pressure.
    loop {
        // Block until at least one event arrives
        let Some(first) = events.next().await else {
            break;
        };

        // Collect first event + drain all already-queued events
        let mut batch = vec![first];
        while let Some(event) = events.try_next() {
            batch.push(event);
        }

        // Process all events
        let mut should_quit = false;
        for event in batch {
            match event {
                AppEvent::Key(key) => {
                    let action = handle_key(&mut app, key);
                    match action {
                        InputAction::Quit => {
                            app.state = AppState::Exiting;
                            should_quit = true;
                            break;
                        }
                        InputAction::Submit(text) => {
                            let _ = user_input_tx.send(UserCommand::Message(text)).await;
                        }
                        InputAction::InboxPush(text) => {
                            app.push_to_inbox(text);
                            if let Some(msg) = app.try_forward_inbox() {
                                let _ =
                                    user_input_tx.send(UserCommand::Message(msg)).await;
                            }
                        }
                        InputAction::ToolApprove => {
                            if let AppState::ToolConfirm { ref id, .. } = app.state {
                                let id = id.clone();
                                app.state = AppState::Running;
                                let _ = permission_tx.send((id, true)).await;
                            }
                        }
                        InputAction::ToolDeny => {
                            if let AppState::ToolConfirm { ref id, .. } = app.state {
                                let id = id.clone();
                                app.state = AppState::Running;
                                let _ = permission_tx.send((id, false)).await;
                            }
                        }
                        InputAction::ModeSwitch(mode) => {
                            app.mode = mode.clone();
                            let new_mode = match mode.as_str() {
                                "plan" => AgentMode::Plan,
                                _ => AgentMode::Act,
                            };
                            let _ = user_input_tx.send(UserCommand::ModeSwitch(new_mode)).await;
                        }
                        InputAction::SlashCommand(cmd) => {
                            handle_slash_command(&mut app, cmd, &user_input_tx).await;
                        }
                        InputAction::None => {}
                    }
                }
                AppEvent::Agent(agent_event) => {
                    app.handle_agent_event(agent_event);
                    // AwaitingInput sets agent_idle=true — try forwarding queued Inbox messages
                    if let Some(msg) = app.try_forward_inbox() {
                        let _ = user_input_tx.send(UserCommand::Message(msg)).await;
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

        if should_quit || app.state == AppState::Exiting {
            break;
        }

        // Single draw after processing all batched events
        terminal.draw(|f| draw(f, &app))?;
    }

    // Explicit cleanup (guard will also clean up in Drop as a safety net)
    terminal.show_cursor()?;

    Ok(())
}
