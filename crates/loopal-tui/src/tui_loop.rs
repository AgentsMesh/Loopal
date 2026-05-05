use std::io;

use ratatui::prelude::*;

use loopal_protocol::{AgentEvent, AgentEventPayload};
use tokio::sync::mpsc;

use crate::app::{App, SubPage};
use crate::event::{AppEvent, EventHandler};
use crate::input::paste;
use crate::key_dispatch::handle_key_action;
use crate::render::draw;
use crate::resume_display::{load_resumed_display, push_session_warning};
use crate::terminal::TerminalGuard;
use crate::tui_sync::sync_panel_caches;

/// `app` is supplied pre-initialized so the bootstrap can apply
/// optimistic display updates before the event loop starts.
pub async fn run_tui(
    mut app: App,
    agent_event_rx: mpsc::Receiver<AgentEvent>,
    resync_rx: mpsc::Receiver<()>,
) -> anyhow::Result<()> {
    crate::terminal::install_panic_hook();
    let _guard = TerminalGuard::new()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let events = EventHandler::new(agent_event_rx, resync_rx);

    run_tui_loop(&mut terminal, events, &mut app).await?;

    terminal.show_cursor()?;
    Ok(())
}

pub async fn run_tui_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    mut events: EventHandler,
    app: &mut App,
) -> anyhow::Result<()>
where
    B::Error: Send + Sync + 'static,
{
    sync_panel_caches(app);
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
        let mut should_resync = false;
        for event in batch {
            match event {
                AppEvent::ScrollUp => app.content_scroll.scroll_up(3),
                AppEvent::ScrollDown => app.content_scroll.scroll_down(3),
                AppEvent::Key(key) => {
                    should_quit = handle_key_action(app, key, &events).await;
                    if should_quit {
                        break;
                    }
                }
                AppEvent::Agent(agent_event) => handle_agent_event(app, agent_event),
                AppEvent::Paste(result) => paste::apply_paste_result(app, result),
                AppEvent::Resync => should_resync = true,
                AppEvent::Resize(_, _) | AppEvent::Tick => {}
            }
        }

        if should_quit || app.exiting {
            break;
        }
        if should_resync {
            app.resync_view_clients().await;
        }
        sync_panel_caches(app);
        terminal.draw(|f| draw(f, app))?;
    }

    Ok(())
}

fn handle_agent_event(app: &mut App, agent_event: AgentEvent) {
    if let AgentEventPayload::SessionResumed { ref session_id, .. } = agent_event.payload {
        load_resumed_display(app, session_id);
    }
    if let AgentEventPayload::SessionResumeWarnings { ref warnings, .. } = agent_event.payload {
        for w in warnings {
            push_session_warning(app, w);
        }
    }
    let is_mcp_report = matches!(
        agent_event.payload,
        AgentEventPayload::McpStatusReport { .. }
    );
    app.dispatch_event(agent_event);
    if is_mcp_report {
        refresh_mcp_page(app);
    }
}

fn refresh_mcp_page(app: &mut App) {
    if let Some(SubPage::McpPage(ref mut state)) = app.sub_page {
        let servers = app.session.lock().mcp_status.clone().unwrap_or_default();
        state.selected = state.selected.min(servers.len().saturating_sub(1));
        state.scroll_offset = state.scroll_offset.min(servers.len().saturating_sub(1));
        state.servers = servers;
        state.loaded = true;
        state.action_menu = None;
    }
}
