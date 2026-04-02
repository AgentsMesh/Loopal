//! Agent server entry point — stdio-only IPC lifecycle + agent loop.
//! Activated internally via hidden `--serve` flag. Communicates with Hub via stdin/stdout.
//! Agent is a pure worker: no TCP listener, no server_info, no external ports.

use std::sync::Arc;

use tracing::info;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_ipc::{StdioTransport, jsonrpc};

use crate::server_init::wait_for_initialize_with_token;
use crate::session_hub::SessionHub;

/// Run the agent server over stdio (pure worker, no TCP listener).
pub async fn run_agent_server() -> anyhow::Result<()> {
    info!("agent server starting (stdio mode)");
    let transport: Arc<dyn Transport> = Arc::new(StdioTransport::from_std());
    let connection = Arc::new(Connection::new(transport));
    let incoming_rx = connection.start();
    let hub = Arc::new(SessionHub::new());
    run_connection(connection, incoming_rx, &hub).await
}

/// Run the agent server with mock provider (for system tests).
pub async fn run_agent_server_with_mock(mock_path: &str) -> anyhow::Result<()> {
    info!(mock_path, "agent server starting with mock provider");
    let provider = crate::mock_loader::load_mock_provider(mock_path)?;
    let cwd = std::env::current_dir().unwrap_or_default();
    let session_dir = std::env::temp_dir().join("loopal-test-sessions");
    let transport: Arc<dyn Transport> = Arc::new(StdioTransport::from_std());
    crate::test_server::run_server_for_test(transport, provider, cwd, session_dir).await
}

async fn run_connection(
    connection: Arc<Connection>,
    mut incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    hub: &SessionHub,
) -> anyhow::Result<()> {
    wait_for_initialize_with_token(&connection, &mut incoming_rx, None).await?;
    dispatch_loop(connection, incoming_rx, hub, true).await
}

/// Permanent dispatch loop. Routes messages to the active session or
/// handles lifecycle commands (agent/start, agent/shutdown).
pub(crate) async fn dispatch_loop(
    connection: Arc<Connection>,
    mut incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    hub: &SessionHub,
    is_production: bool,
) -> anyhow::Result<()> {
    loop {
        let Some(msg) = incoming_rx.recv().await else {
            info!("connection closed");
            break;
        };
        match msg {
            Incoming::Request { id, method, params } => {
                if method == methods::AGENT_START.name {
                    let agent_output = run_session(
                        &connection,
                        &mut incoming_rx,
                        hub,
                        is_production,
                        id,
                        params,
                    )
                    .await?;
                    if agent_output.is_some() {
                        // Prompt-driven session complete — send result and exit.
                        send_agent_completed(&connection, agent_output.as_ref()).await;
                        break;
                    }
                } else if method == methods::AGENT_SHUTDOWN.name {
                    let _ = connection
                        .respond(id, serde_json::json!({"ok": true}))
                        .await;
                    break;
                } else if method == methods::AGENT_LIST.name {
                    let ids = hub.list_session_ids().await;
                    let sessions: Vec<_> = ids
                        .iter()
                        .map(|id| serde_json::json!({"session_id": id}))
                        .collect();
                    let _ = connection.respond(id, serde_json::json!(sessions)).await;
                } else {
                    let _ = connection
                        .respond_error(id, jsonrpc::METHOD_NOT_FOUND, "expected agent/start")
                        .await;
                }
            }
            Incoming::Notification { .. } => {}
        }
    }
    // Send completion for non-prompt sessions (prompt sessions send above).
    send_agent_completed(&connection, None).await;
    info!("server shutting down");
    Ok(())
}

/// Run one session (with possible chained restarts). Returns `Some(output)` if
/// the session was prompt-driven (server should exit), `None` otherwise.
async fn run_session(
    connection: &Arc<Connection>,
    incoming_rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    hub: &SessionHub,
    is_production: bool,
    id: i64,
    params: serde_json::Value,
) -> anyhow::Result<Option<loopal_error::AgentOutput>> {
    let mut handle =
        crate::session_start::start_session(connection, id, params, hub, is_production).await?;
    let mut forward_result =
        crate::session_forward::forward_loop(incoming_rx, connection, &mut handle).await;
    hub.remove_session(&handle.session_id).await;

    // Handle chained agent/start requests.
    while let crate::session_forward::ForwardResult::NewStart {
        id: new_id,
        params: new_params,
    } = forward_result
    {
        info!("chained agent/start after session end");
        handle =
            crate::session_start::start_session(connection, new_id, new_params, hub, is_production)
                .await?;
        forward_result =
            crate::session_forward::forward_loop(incoming_rx, connection, &mut handle).await;
        hub.remove_session(&handle.session_id).await;
    }

    let agent_output = match forward_result {
        crate::session_forward::ForwardResult::Done(output) => output,
        _ => None,
    };

    if handle.lifecycle == loopal_runtime::LifecycleMode::Ephemeral {
        info!("ephemeral session complete, server exiting");
        Ok(agent_output)
    } else {
        info!("persistent session ended, ready for next");
        Ok(None)
    }
}

/// Send `agent/completed` with the authoritative agent output.
async fn send_agent_completed(connection: &Connection, output: Option<&loopal_error::AgentOutput>) {
    let (reason, result) = match output {
        Some(o) => (o.terminate_reason.as_str(), serde_json::json!(o.result)),
        None => ("shutdown", serde_json::Value::Null),
    };
    let _ = connection
        .send_notification(
            methods::AGENT_COMPLETED.name,
            serde_json::json!({"reason": reason, "result": result}),
        )
        .await;
}
