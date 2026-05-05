use std::io;
use std::sync::atomic::Ordering;

use ratatui::prelude::*;

use loopal_protocol::{AgentEvent, AgentEventPayload};
use tokio::sync::mpsc;

use crate::app::{App, HubReconnectInfo, SubPage};
use crate::event::{AppEvent, EventHandler};
use crate::input::paste;
use crate::key_dispatch::handle_key_action;
use crate::render::draw;
use crate::resume_display::{load_resumed_display, push_session_warning};
use crate::terminal::TerminalGuard;
use crate::tui_sync::sync_panel_caches;

/// Surfaced to bootstrap after `run_tui` returns. `detach_requested` lets
/// bootstrap distinguish a `/detach-hub` exit (print re-attach instructions)
/// from a `/exit` or `/kill-hub` exit (silent / shutdown). `connection_lost`
/// indicates Hub TCP closed before the user issued an exit command.
/// `shutdown_initiated` distinguishes user-driven shutdown (silent) from
/// an unexpected Hub crash (loud "lost" warning).
pub struct ExitInfo {
    pub detach_requested: bool,
    pub reconnect_info: Option<HubReconnectInfo>,
    pub connection_lost: bool,
    pub shutdown_initiated: bool,
}

/// `app` is supplied pre-initialized so the bootstrap can apply
/// optimistic display updates before the event loop starts.
pub async fn run_tui(
    mut app: App,
    agent_event_rx: mpsc::Receiver<AgentEvent>,
    resync_rx: mpsc::Receiver<()>,
) -> anyhow::Result<ExitInfo> {
    crate::terminal::install_panic_hook();
    let _guard = TerminalGuard::new()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let events = EventHandler::new(agent_event_rx, resync_rx);

    run_tui_loop(&mut terminal, events, &mut app).await?;

    terminal.show_cursor()?;
    let connection_lost = app.hub_connection_lost.load(Ordering::Relaxed);
    Ok(ExitInfo {
        detach_requested: app.detach_requested,
        reconnect_info: app.hub_reconnect_info.clone(),
        connection_lost,
        shutdown_initiated: app.shutdown_initiated,
    })
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
