//! Default mode — Hub-first multi-process architecture.
//!
//! Flow: Start Hub → spawn root agent → connect TUI via UiSession.

use tracing::info;

use loopal_agent_hub::UiSession;
use loopal_runtime::projection::project_messages;
use loopal_session::SessionController;

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    info!("starting in Hub mode");

    // 1-3. Create Hub + spawn root agent
    let ctx = super::common::bootstrap_hub_and_agent(cli, cwd, config).await?;

    // 4. Start event broadcast
    let _event_loop = loopal_agent_hub::start_event_loop(ctx.hub.clone(), ctx.event_rx);

    // 5. Connect TUI as UI client (one line — all wiring inside UiSession)
    let ui_session = UiSession::connect(ctx.hub.clone(), "tui").await;
    info!("TUI connected to Hub as UI client");

    // 6. Bridge broadcast → mpsc for TUI event handler
    let tui_event_rx = bridge_broadcast_to_mpsc(ui_session.event_rx);

    // 7. Build SessionController
    let model = config.settings.model.clone();
    let mode_str = if cli.plan { "plan" } else { "act" };
    let session_ctrl = SessionController::with_hub(
        model.clone(),
        mode_str.to_string(),
        ui_session.client.clone(),
        ctx.hub.clone(),
    );

    // 8. Handle permission/question relay from Hub
    let session_for_relay = session_ctrl.clone();
    tokio::spawn(async move {
        handle_tui_incoming(session_for_relay, ui_session.relay_rx).await;
    });

    // 9. Load display history or show welcome
    if let Some(ref sid) = cli.resume {
        let session_manager = loopal_runtime::SessionManager::new()?;
        if let Ok((_session, messages)) = session_manager.resume_session(sid) {
            session_ctrl.load_display_history(project_messages(&messages));
        }
    } else {
        let display_path = super::abbreviate_home(cwd);
        session_ctrl.push_welcome(&model, &display_path);
    }

    // 10. Run TUI
    let result = loopal_tui::run_tui(session_ctrl, cwd.to_path_buf(), tui_event_rx).await;

    // 11. Cleanup
    info!("shutting down agent process");
    let _ = ctx.agent_proc.shutdown().await;

    result
}

/// Bridge broadcast::Receiver → mpsc::Receiver for TUI compatibility.
fn bridge_broadcast_to_mpsc(
    mut broadcast_rx: tokio::sync::broadcast::Receiver<loopal_protocol::AgentEvent>,
) -> tokio::sync::mpsc::Receiver<loopal_protocol::AgentEvent> {
    let (tx, rx) = tokio::sync::mpsc::channel(4096);
    tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(event) => {
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(skipped = n, "TUI event bridge lagged");
                }
            }
        }
    });
    rx
}

/// Handle incoming relay requests from Hub via UiSession.
async fn handle_tui_incoming(
    session: SessionController,
    mut rx: tokio::sync::mpsc::Receiver<loopal_ipc::connection::Incoming>,
) {
    use loopal_ipc::connection::Incoming;
    info!("TUI incoming handler started");
    while let Some(msg) = rx.recv().await {
        if let Incoming::Request { id, method, .. } = msg {
            if method == loopal_ipc::protocol::methods::AGENT_PERMISSION.name {
                {
                    let mut state = session.lock();
                    if let Some(ref mut perm) = state.pending_permission {
                        perm.relay_request_id = Some(id);
                    }
                }
                info!(%method, id, "TUI auto-approving permission");
                session.approve_permission().await;
            } else if method == loopal_ipc::protocol::methods::AGENT_QUESTION.name {
                {
                    let mut state = session.lock();
                    if let Some(ref mut q) = state.pending_question {
                        q.relay_request_id = Some(id);
                    }
                }
                info!(%method, id, "TUI auto-approving question");
                session
                    .answer_question(vec!["(auto-approved)".into()])
                    .await;
            }
        }
    }
    info!("TUI incoming handler exited");
}
