use std::sync::Arc;

use tracing::info;

use loopal_agent_hub::HubClient;
use loopal_context::project_messages_to_display;
use loopal_session::SessionController;

use super::attach_bridge::{RegisteredUi, bridge_events, connect_and_register};
use crate::cli::Cli;

pub struct AttachOutcome {
    pub session_id: Option<String>,
    pub detached: bool,
}

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    hub_addr: &str,
) -> anyhow::Result<()> {
    let token = cli
        .parent_only
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
    let ui = connect_and_register(hub_addr, hub_token).await?;
    run_with_registered_ui(
        cwd,
        config,
        hub_addr,
        hub_token,
        root_session_id,
        resume,
        ui,
    )
    .await
}

pub async fn run_with_registered_ui(
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    hub_addr: &str,
    hub_token: &str,
    root_session_id: Option<&str>,
    resume: Option<&str>,
    ui: RegisteredUi,
) -> anyhow::Result<AttachOutcome> {
    let RegisteredUi { conn, incoming_rx } = ui;
    let lease_conn = conn.clone();

    let mut app = loopal_tui::app::App::new(
        SessionController::with_hub(Arc::new(HubClient::new_with_transport_lease(conn))),
        cwd.to_path_buf(),
    );
    app.hub_reconnect_info = Some(loopal_tui::app::HubReconnectInfo {
        addr: hub_addr.to_string(),
        token: hub_token.to_string(),
    });
    let (event_rx, resync_rx) = bridge_events(incoming_rx, app.hub_connection_lost.clone());

    let session_ctrl = app.session.clone();
    if let Some(sid) = root_session_id {
        session_ctrl.set_root_session_id(sid);
    }
    if let Err(e) = app.seed_view_clients().await {
        tracing::warn!(error = %e, "view/snapshot seed failed, continuing with empty view_clients");
    }
    seed_resume_or_welcome(&mut app, &config.settings.model, cwd, resume);

    let tui_result = loopal_tui::run_tui(app, event_rx, resync_rx).await;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), lease_conn.close()).await;
    let exit_info = tui_result?;
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
            Ok((session_manager, (session, turns))) => {
                let messages = loopal_provider_api::project_turns_to_messages(&turns);
                app.load_display_history(project_messages_to_display(&messages));
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
