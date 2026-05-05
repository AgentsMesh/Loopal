use std::sync::Arc;

use tracing::info;

use loopal_agent_hub::HubClient;
use loopal_protocol::project_messages;
use loopal_session::SessionController;

use super::attach_bridge::{bridge_events, connect_and_register};
use crate::cli::Cli;

/// Outcome of a TUI attach session — used by `multiprocess` to decide
/// whether to print resume instructions on exit.
pub struct AttachOutcome {
    /// `Some(id)` if SessionController has a known root session;
    /// `None` only for pure `--attach-hub` runs that never owned the root.
    pub session_id: Option<String>,
    /// `true` when the user exited via `/detach-hub` (Hub stays alive,
    /// re-attach instructions were already printed).
    pub detached: bool,
}

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    hub_addr: &str,
) -> anyhow::Result<()> {
    let token = cli
        .hub_token
        .clone()
        .ok_or_else(|| anyhow::anyhow!("--attach-hub requires --hub-token"))?;
    let _ = run_with_addr(cwd, config, hub_addr, &token, None, None).await?;
    Ok(())
}

pub async fn run_with_addr(
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    hub_addr: &str,
    hub_token: &str,
    root_session_id: Option<&str>,
    resume: Option<&str>,
) -> anyhow::Result<AttachOutcome> {
    info!(hub = %hub_addr, "TUI attach: connecting");
    let (conn, incoming_rx) = connect_and_register(hub_addr, hub_token).await?;

    let mut app = loopal_tui::app::App::new(
        SessionController::with_hub(Arc::new(HubClient::new(conn))),
        cwd.to_path_buf(),
    );
    app.hub_reconnect_info = Some(loopal_tui::app::HubReconnectInfo {
        addr: hub_addr.to_string(),
        token: hub_token.to_string(),
    });
    let connection_lost = app.hub_connection_lost.clone();
    let (event_rx, resync_rx) = bridge_events(incoming_rx, connection_lost);

    let session_ctrl = app.session.clone();
    if let Some(sid) = root_session_id {
        session_ctrl.set_root_session_id(sid);
    }
    if let Err(e) = app.seed_view_clients().await {
        tracing::warn!(error = %e, "view/snapshot seed failed, continuing with empty view_clients");
    }
    seed_resume_or_welcome(&mut app, &config.settings.model, cwd, resume);

    let exit_info = loopal_tui::run_tui(app, event_rx, resync_rx).await?;
    let detached = exit_info.detach_requested;
    print_post_exit_message(&exit_info);

    let session_id = session_ctrl
        .root_session_id()
        .or_else(|| root_session_id.map(String::from));
    Ok(AttachOutcome {
        session_id,
        detached,
    })
}

fn print_post_exit_message(exit: &loopal_tui::ExitInfo) {
    if exit.detach_requested && !exit.connection_lost {
        if let Some(info) = exit.reconnect_info.as_ref() {
            eprintln!();
            eprintln!("Detached from Hub. Hub and agents continue running.");
            eprintln!("To re-attach:");
            eprintln!(
                "  loopal --attach-hub {} --hub-token {}",
                info.addr, info.token
            );
        }
    } else if exit.connection_lost && !exit.shutdown_initiated {
        // Only warn about an unexpected disconnect when the user did
        // NOT just ask Hub to shut down (`/exit` / `/kill-hub`). A
        // post-shutdown TCP close is the expected behaviour, not a
        // crash worth alarming the user about.
        eprintln!();
        eprintln!("Hub connection lost. The Hub process may have exited or crashed.");
    }
}

fn seed_resume_or_welcome(
    app: &mut loopal_tui::app::App,
    model: &str,
    cwd: &std::path::Path,
    resume: Option<&str>,
) {
    if let Some(sid) = resume {
        match loopal_runtime::SessionManager::new()
            .and_then(|sm| sm.resume_session(sid).map(|p| (sm, p)))
        {
            Ok((session_manager, (session, messages))) => {
                app.load_display_history(project_messages(&messages));
                super::sub_agent_resume::load_sub_agent_histories(app, &session, &session_manager);
            }
            Err(e) => {
                tracing::warn!(session_id = sid, error = %e, "failed to resume session");
                let short = &sid[..8.min(sid.len())];
                app.push_system_message(format!("Failed to resume session {short}: {e}"));
            }
        }
        return;
    }
    let display_path = super::abbreviate_home(cwd);
    app.push_welcome(model, &display_path);
}
