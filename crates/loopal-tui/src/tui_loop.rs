use std::io;
use std::path::PathBuf;

use ratatui::prelude::*;

use loopal_protocol::AgentEvent;
use loopal_session::SessionController;
use tokio::sync::mpsc;

use crate::app::App;
use crate::event::{AppEvent, EventHandler};
use crate::input::paste;
use crate::key_dispatch::handle_key_action;
use crate::render::draw;
use crate::terminal::TerminalGuard;
use crate::tui_helpers::route_human_message;

/// Run the TUI event loop with a real terminal (production entry point).
pub async fn run_tui(
    session: SessionController,
    cwd: PathBuf,
    agent_event_rx: mpsc::Receiver<AgentEvent>,
) -> anyhow::Result<()> {
    let _guard = TerminalGuard::new()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let events = EventHandler::new(agent_event_rx);
    let mut app = App::new(session, cwd);

    run_tui_loop(&mut terminal, events, &mut app).await?;

    terminal.show_cursor()?;
    Ok(())
}

/// Backend-agnostic TUI event loop.
pub async fn run_tui_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    mut events: EventHandler,
    app: &mut App,
) -> anyhow::Result<()>
where
    B::Error: Send + Sync + 'static,
{
    terminal.draw(|f| draw(f, app))?;

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
                    should_quit = handle_key_action(app, key, &events).await;
                    if should_quit {
                        break;
                    }
                }
                AppEvent::Agent(agent_event) => {
                    if let Some(content) = app.session.handle_event(agent_event) {
                        route_human_message(app, content).await;
                    }
                }
                AppEvent::Paste(result) => {
                    paste::apply_paste_result(app, result);
                }
                AppEvent::Resize(_, _) | AppEvent::Tick => {}
            }
        }

        if should_quit || app.exiting {
            break;
        }
        terminal.draw(|f| draw(f, app))?;
    }

    Ok(())
}
