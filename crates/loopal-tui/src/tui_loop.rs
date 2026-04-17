//! TUI event loop — main run loop for the terminal UI.

use std::io;
use std::path::PathBuf;

use ratatui::prelude::*;

use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_session::SessionController;
use tokio::sync::mpsc;

use crate::app::{App, SubPage};
use crate::event::{AppEvent, EventHandler};
use crate::input::paste;
use crate::key_dispatch::handle_key_action;
use crate::render::draw;
use crate::terminal::TerminalGuard;

/// Run the TUI event loop with a real terminal (production entry point).
pub async fn run_tui(
    session: SessionController,
    cwd: PathBuf,
    agent_event_rx: mpsc::Receiver<AgentEvent>,
) -> anyhow::Result<()> {
    crate::terminal::install_panic_hook();
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
        for event in batch {
            match event {
                AppEvent::ScrollUp => {
                    app.content_scroll.scroll_up(3);
                }
                AppEvent::ScrollDown => {
                    app.content_scroll.scroll_down(3);
                }
                AppEvent::Key(key) => {
                    should_quit = handle_key_action(app, key, &events).await;
                    if should_quit {
                        break;
                    }
                }
                AppEvent::Agent(agent_event) => {
                    if let AgentEventPayload::SessionResumed { ref session_id, .. } =
                        agent_event.payload
                    {
                        load_resumed_display(app, session_id);
                    }
                    let is_mcp_report = matches!(
                        agent_event.payload,
                        AgentEventPayload::McpStatusReport { .. }
                    );
                    app.session.handle_event(agent_event);
                    if is_mcp_report {
                        refresh_mcp_page(app);
                    }
                }
                AppEvent::Paste(result) => {
                    paste::apply_paste_result(app, result);
                }
                AppEvent::Resize(_, _) => {}
                AppEvent::Tick => {}
            }
        }

        if should_quit || app.exiting {
            break;
        }
        sync_panel_caches(app);
        terminal.draw(|f| draw(f, app))?;
    }

    Ok(())
}

/// Refresh the MCP sub-page data if it's currently open.
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

/// Sync all panel caches from session state into App-level fields.
///
/// Copies bg_tasks, task_snapshots, and cron_snapshots. `bg_task_details`
/// uses merge-and-retain so the log viewer can still access completed
/// tasks after they're cleaned from session state.
fn sync_panel_caches(app: &mut App) {
    {
        let mut state = app.session.lock();
        app.bg_snapshots = state.bg_tasks.values().map(|t| t.to_snapshot()).collect();
        crate::session_cleanup::merge_bg_details(&mut app.bg_task_details, &state.bg_tasks);
        app.task_snapshots = state.task_snapshots.clone();
        app.cron_snapshots = state.cron_snapshots.clone();
        crate::session_cleanup::cleanup_session_state(&mut state);
    }
    crate::session_cleanup::cap_bg_details(&mut app.bg_task_details);
    clamp_scroll_offsets(app);
}

/// Ensure scroll offsets don't exceed item counts after data changes.
fn clamp_scroll_offsets(app: &mut App) {
    let clamps: Vec<_> = app
        .panel_registry
        .providers()
        .iter()
        .map(|p| (p.kind(), p.item_ids(app).len(), p.max_visible()))
        .collect();
    for (kind, count, max) in clamps {
        let section = app.section_mut(kind);
        section.scroll_offset = section.scroll_offset.min(count.saturating_sub(max));
    }
}

/// Load display history from storage after the agent confirms a session resume.
fn load_resumed_display(app: &mut App, session_id: &str) {
    let Ok(sm) = loopal_runtime::SessionManager::new() else {
        return;
    };
    let Ok((session, messages)) = sm.resume_session(session_id) else {
        return;
    };
    let projected = loopal_protocol::project_messages(&messages);
    app.session.load_display_history(projected);

    // Restore sub-agent conversation views
    for sub in &session.sub_agents {
        let Ok(sub_msgs) = sm.load_messages(&sub.session_id) else {
            continue;
        };
        if sub_msgs.is_empty() {
            continue;
        }
        app.session.load_sub_agent_history(
            &sub.name,
            &sub.session_id,
            sub.parent.as_deref(),
            sub.model.as_deref(),
            loopal_protocol::project_messages(&sub_msgs),
        );
    }
}
