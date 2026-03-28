//! Default mode — Hub-first multi-process architecture.
//!
//! Flow: Start Hub → spawn root agent (stdio registered as "main") →
//! start TUI via local Hub connection. All agents communicate through Hub.

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;

use loopal_agent_hub::AgentHub;
use loopal_agent_hub::hub_server;
use loopal_runtime::projection::project_messages;
use loopal_session::SessionController;

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    info!("starting in Hub mode");

    // 1. Create Hub
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(256);
    let hub = Arc::new(Mutex::new(AgentHub::new(event_tx)));

    // 2. Start Hub TCP listener for external clients
    let (listener, _hub_port, hub_token) = hub_server::start_hub_listener(hub.clone()).await?;
    let hub_accept = hub.clone();
    tokio::spawn(async move {
        hub_server::accept_loop(listener, hub_accept, hub_token).await;
    });

    // 3. Spawn root agent — register its stdio as "main" in Hub
    let agent_proc = loopal_agent_client::AgentProcess::spawn(None).await?;
    let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
    client.initialize().await?;

    let mode_str = if cli.plan { "plan" } else { "act" };
    let prompt = if cli.prompt.is_empty() {
        None
    } else {
        Some(cli.prompt.join(" "))
    };
    client
        .start_agent(
            cwd,
            Some(&config.settings.model),
            Some(mode_str),
            prompt.as_deref(),
            cli.permission.as_deref(),
            cli.no_sandbox,
            cli.resume.as_deref(),
        )
        .await?;

    // Register root agent's stdio Connection as "main" in Hub
    let (root_conn, incoming_rx) = client.into_parts();
    loopal_agent_hub::agent_io::start_agent_io(hub.clone(), "main", root_conn, incoming_rx, true);
    info!("root agent registered as 'main' in Hub");

    // 4. Create local Hub Connection for TUI (receives permission/question relays)
    let (tui_hub_conn, tui_hub_rx) = hub_server::connect_local(hub.clone(), "_tui");
    info!("TUI connected to Hub as '_tui'");

    // Handle permission/question requests from Hub in background
    let tui_conn_for_relay = tui_hub_conn.clone();
    tokio::spawn(async move {
        handle_tui_incoming(tui_conn_for_relay, tui_hub_rx).await;
    });

    // 5. Event routing: Hub event_tx → frontend → TUI
    let (frontend_tx, frontend_rx) = tokio::sync::mpsc::channel(256);
    let _event_loop = loopal_agent_hub::start_event_loop(hub.clone(), event_rx, frontend_tx);

    // 6. Build SessionController with Hub backend
    let model = config.settings.model.clone();
    let session_ctrl = SessionController::with_hub(
        model.clone(),
        mode_str.to_string(),
        tui_hub_conn,
        hub.clone(),
    );

    // 7. Load display history or show welcome
    if let Some(ref sid) = cli.resume {
        let session_manager = loopal_runtime::SessionManager::new()?;
        if let Ok((_session, messages)) = session_manager.resume_session(sid) {
            session_ctrl.load_display_history(project_messages(&messages));
        }
    } else {
        let display_path = super::abbreviate_home(cwd);
        session_ctrl.push_welcome(&model, &display_path);
    }

    // 8. Run TUI
    let result = loopal_tui::run_tui(session_ctrl, cwd.to_path_buf(), frontend_rx).await;

    // 9. Cleanup
    info!("shutting down agent process");
    let _ = agent_proc.shutdown().await;

    result
}

/// Handle incoming requests from Hub on the TUI's local connection.
/// Auto-approves permissions — real UI permission handling is TODO.
async fn handle_tui_incoming(
    conn: Arc<loopal_ipc::connection::Connection>,
    mut rx: tokio::sync::mpsc::Receiver<loopal_ipc::connection::Incoming>,
) {
    use loopal_ipc::connection::Incoming;
    info!("TUI incoming handler started");
    while let Some(msg) = rx.recv().await {
        if let Incoming::Request { id, method, .. } = msg {
            if method == loopal_ipc::protocol::methods::AGENT_PERMISSION.name {
                info!(%method, id, "TUI auto-approving permission");
                let _ = conn.respond(id, serde_json::json!({"allow": true})).await;
            } else if method == loopal_ipc::protocol::methods::AGENT_QUESTION.name {
                info!(%method, id, "TUI auto-approving question");
                let _ = conn
                    .respond(id, serde_json::json!({"answers": ["(auto-approved)"]}))
                    .await;
            }
        }
    }
    info!("TUI incoming handler exited");
}
