//! Multi-process mode (default) — TUI spawns Agent as child process.

use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::InterruptSignal;
use loopal_session::SessionController;
use loopal_session::connection_manager::{AgentConnectionManager, PrimaryConn};

use crate::cli::Cli;

pub async fn run(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<()> {
    tracing::info!("starting in multi-process mode");
    let agent_proc = loopal_agent_client::AgentProcess::spawn(None).await?;
    let result = run_with_agent(cli, cwd, config, &agent_proc).await;
    tracing::info!("shutting down agent process");
    let _ = agent_proc.shutdown().await;
    result
}

async fn run_with_agent(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    agent_proc: &loopal_agent_client::AgentProcess,
) -> anyhow::Result<()> {
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
        )
        .await?;

    let (connection, incoming_rx) = client.into_parts();
    let handles = loopal_agent_client::start_bridge(connection.clone(), incoming_rx);

    let interrupt = InterruptSignal::new();
    let (watch_tx, _watch_rx) = tokio::sync::watch::channel(0u64);
    let interrupt_tx = Arc::new(watch_tx);

    // Forward local interrupt signal to Agent process via IPC
    tokio::spawn(forward_interrupt(
        interrupt.clone(),
        interrupt_tx.subscribe(),
        connection,
    ));

    let model = config.settings.model.clone();

    let primary = PrimaryConn {
        control_tx: handles.control_tx,
        permission_tx: handles.permission_tx,
        question_tx: handles.question_tx,
        mailbox_tx: Some(handles.mailbox_tx),
        interrupt: interrupt.clone(),
        interrupt_tx: interrupt_tx.clone(),
    };

    let manager = AgentConnectionManager::new(handles.agent_event_tx.clone());

    let session_ctrl =
        SessionController::with_primary(model.clone(), mode_str.to_string(), primary, manager);

    let display_path = super::abbreviate_home(cwd);
    session_ctrl.push_welcome(&model, &display_path);

    loopal_tui::run_tui(session_ctrl, cwd.to_path_buf(), handles.agent_event_rx).await
}

async fn forward_interrupt(
    signal: InterruptSignal,
    mut rx: tokio::sync::watch::Receiver<u64>,
    connection: Arc<Connection>,
) {
    while rx.changed().await.is_ok() {
        if signal.take() {
            tracing::debug!("forwarding interrupt to agent process");
            let _ = connection
                .send_notification(methods::AGENT_INTERRUPT.name, serde_json::Value::Null)
                .await;
        }
    }
}
