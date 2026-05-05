//! Default mode — Hub-first multi-process architecture.
//!
//! Flow: Start Hub → spawn root agent → connect TUI via UiSession.

use tracing::info;

use loopal_agent_hub::UiSession;
use loopal_protocol::project_messages;
use loopal_session::SessionController;

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
) -> anyhow::Result<String> {
    info!("starting in Hub mode");

    let ctx = super::hub_bootstrap::bootstrap_hub_and_agent(cli, cwd, config, resume).await?;
    let root_session_id = ctx.root_session_id.clone();

    if let Some(port) = ctx.hub.lock().await.listener_port {
        eprintln!("Hub listening on 127.0.0.1:{port}");
        eprintln!("To attach a second TUI:");
        eprintln!(
            "  loopal --attach-hub 127.0.0.1:{port} --hub-token {}",
            ctx.hub_token
        );
    }

    let _event_loop = loopal_agent_hub::start_event_loop(ctx.hub.clone(), ctx.event_rx);

    let ui_session = UiSession::connect(ctx.hub.clone(), "tui").await;
    info!("TUI connected to Hub as UI client");

    let (tui_event_rx, resync_rx) = bridge_broadcast_to_mpsc(ui_session.event_rx);

    let model = config.settings.model.clone();
    let session_ctrl = SessionController::with_hub(ui_session.client.clone(), ctx.hub.clone());
    session_ctrl.set_root_session_id(&root_session_id);

    let persist_ctrl = session_ctrl.clone();
    tokio::spawn(async move {
        super::sub_agent_resume::persist_sub_agent_refs_loop(persist_ctrl).await;
    });

    let mut app = loopal_tui::app::App::new(session_ctrl.clone(), cwd.to_path_buf());
    let session_manager = loopal_runtime::SessionManager::new()?;
    if let Some(sid) = resume {
        match session_manager.resume_session(sid) {
            Ok((session, messages)) => {
                app.load_display_history(project_messages(&messages));
                super::sub_agent_resume::load_sub_agent_histories(
                    &mut app,
                    &session,
                    &session_manager,
                );
            }
            Err(e) => {
                tracing::warn!(session_id = sid, error = %e, "failed to resume session");
                let short = &sid[..8.min(sid.len())];
                app.push_system_message(format!("Failed to resume session {short}: {e}"));
            }
        }
    } else {
        let display_path = super::abbreviate_home(cwd);
        app.push_welcome(&model, &display_path);
    }

    let session_ref = session_ctrl.clone();
    let result = loopal_tui::run_tui(app, tui_event_rx, resync_rx).await;

    info!("shutting down agent process");
    let _ = ctx.agent_proc.shutdown().await;

    let final_session_id = session_ref.root_session_id().unwrap_or(root_session_id);
    result.map(|()| final_session_id)
}

fn bridge_broadcast_to_mpsc(
    mut broadcast_rx: tokio::sync::broadcast::Receiver<loopal_protocol::AgentEvent>,
) -> (
    tokio::sync::mpsc::Receiver<loopal_protocol::AgentEvent>,
    tokio::sync::mpsc::Receiver<()>,
) {
    let (tx, rx) = tokio::sync::mpsc::channel(4096);
    let (resync_tx, resync_rx) = tokio::sync::mpsc::channel(8);
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
                    tracing::warn!(skipped = n, "TUI event bridge lagged; signaling resync");
                    let _ = resync_tx.try_send(());
                }
            }
        }
    });
    (rx, resync_rx)
}
