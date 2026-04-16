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

    // 1-3. Create Hub + spawn root agent
    let ctx = super::hub_bootstrap::bootstrap_hub_and_agent(cli, cwd, config, resume).await?;
    let root_session_id = ctx.root_session_id.clone();

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
    session_ctrl.set_root_session_id(&root_session_id);

    // 8. Handle permission/question relay from Hub
    let session_for_relay = session_ctrl.clone();
    tokio::spawn(async move {
        handle_tui_incoming(session_for_relay, ui_session.relay_rx).await;
    });

    // 8b. Background task: persist sub-agent refs to root session metadata
    let persist_ctrl = session_ctrl.clone();
    tokio::spawn(async move {
        super::sub_agent_resume::persist_sub_agent_refs_loop(persist_ctrl).await;
    });

    // 9. Load display history or show welcome
    let session_manager = loopal_runtime::SessionManager::new()?;
    if let Some(sid) = resume {
        match session_manager.resume_session(sid) {
            Ok((session, messages)) => {
                session_ctrl.load_display_history(project_messages(&messages));
                super::sub_agent_resume::load_sub_agent_histories(
                    &session_ctrl,
                    &session,
                    &session_manager,
                );
            }
            Err(e) => {
                tracing::warn!(session_id = sid, error = %e, "failed to resume session");
                let short = &sid[..8.min(sid.len())];
                session_ctrl
                    .push_system_message(format!("Failed to resume session {short}: {e}"));
            }
        }
    } else {
        let display_path = super::abbreviate_home(cwd);
        session_ctrl.push_welcome(&model, &display_path);
    }

    // 10. Run TUI (bg tasks synced from agent process via BgTasksUpdate events)
    // Clone before move into run_tui — used after TUI exits to read final session ID.
    let session_ref = session_ctrl.clone();
    let result = loopal_tui::run_tui(session_ctrl, cwd.to_path_buf(), tui_event_rx).await;

    // 11. Cleanup
    info!("shutting down agent process");
    let _ = ctx.agent_proc.shutdown().await;

    // Read the final session ID — may differ from `root_session_id` if the user
    // switched sessions via `/resume` during the TUI session. The fallback to
    // `root_session_id` is defensive only; `set_root_session_id` on line 44
    // guarantees the Option is always Some.
    let final_session_id = session_ref.root_session_id().unwrap_or(root_session_id);
    result.map(|()| final_session_id)
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
///
/// In auto-approve mode: respond directly using the relay request ID.
/// No dependency on pending_permission being set by the event handler first,
/// avoiding the race between broadcast events and relay requests.
async fn handle_tui_incoming(
    session: SessionController,
    mut rx: tokio::sync::mpsc::Receiver<loopal_ipc::connection::Incoming>,
) {
    use loopal_ipc::connection::Incoming;
    info!("TUI incoming handler started");
    while let Some(msg) = rx.recv().await {
        if let Incoming::Request { id, method, params } = msg {
            let agent_name = params
                .as_object()
                .and_then(|o| o.get("agent_name"))
                .and_then(|v| v.as_str())
                .unwrap_or(loopal_session::ROOT_AGENT);

            if method == loopal_ipc::protocol::methods::AGENT_PERMISSION.name {
                // Auto-approve: respond directly with the relay ID, bypassing
                // the pending_permission → relay_request_id → approve dance.
                // This avoids a race where the relay arrives before the event.
                info!(%method, id, agent = %agent_name, "TUI auto-approving permission");
                session.auto_approve_permission(id).await;
            } else if method == loopal_ipc::protocol::methods::AGENT_QUESTION.name {
                info!(%method, id, agent = %agent_name, "TUI auto-approving question");
                session
                    .auto_answer_question(id, vec!["(auto-approved)".into()])
                    .await;
            }
        }
    }
    info!("TUI incoming handler exited");
}
