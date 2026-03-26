//! Multi-process mode (default) — TUI spawns Agent as child process.

use std::sync::Arc;

use tokio::sync::mpsc;

use loopal_agent::router::MessageRouter;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, InterruptSignal};
use loopal_session::SessionController;

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
    tracing::info!("multi-process shutdown complete");
    result
}

async fn run_with_agent(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    agent_proc: &loopal_agent_client::AgentProcess,
) -> anyhow::Result<()> {
    let transport = agent_proc.transport();

    let client = loopal_agent_client::AgentClient::new(transport);
    client.initialize().await?;

    let mode_str = if cli.plan { "plan" } else { "act" };
    let prompt = if cli.prompt.is_empty() {
        None
    } else {
        Some(cli.prompt.join(" "))
    };
    let perm_str = cli.permission.as_deref();
    client
        .start_agent(
            cwd,
            Some(&config.settings.model),
            Some(mode_str),
            prompt.as_deref(),
            perm_str,
            cli.no_sandbox,
        )
        .await?;

    // Hand Connection to bridge (avoids double reader loop on same Transport)
    let (connection, incoming_rx) = client.into_parts();
    let handles = loopal_agent_client::start_bridge(connection.clone(), incoming_rx);

    let interrupt = InterruptSignal::new();
    let interrupt_tx = Arc::new(tokio::sync::watch::channel(0u64).0);

    // Forward local interrupt signal to Agent process via IPC
    let interrupt_for_bridge = interrupt.clone();
    let interrupt_rx = interrupt_tx.subscribe();
    let interrupt_conn = connection.clone();
    tokio::spawn(forward_interrupt(interrupt_for_bridge, interrupt_rx, interrupt_conn));

    let model = config.settings.model.clone();

    let (observation_tx, _) = mpsc::channel::<AgentEvent>(16);
    let router = Arc::new(MessageRouter::new(observation_tx));
    router
        .register("main", handles.mailbox_tx)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let session_ctrl = SessionController::new(
        model.clone(),
        mode_str.to_string(),
        handles.control_tx,
        handles.permission_tx,
        handles.question_tx,
        interrupt,
        interrupt_tx,
    );

    let display_path = super::abbreviate_home(cwd);
    session_ctrl.push_welcome(&model, &display_path);

    loopal_tui::run_tui(
        session_ctrl, router, "main".to_string(), cwd.to_path_buf(),
        handles.agent_event_rx,
    )
    .await
}

/// Forward interrupt signal from TUI to Agent process via IPC notification.
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
