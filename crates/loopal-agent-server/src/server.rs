//! Agent server entry point — IPC lifecycle + agent loop.
//! Activated via `loopal --serve`. Optionally starts a TCP listener.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_ipc::{StdioTransport, jsonrpc};

use crate::server_info;
use crate::session_hub::SessionHub;
use crate::tcp_accept;

#[derive(Deserialize)]
struct InitializeParams {
    #[serde(default)]
    #[allow(dead_code)]
    protocol_version: u32,
}

#[derive(Serialize)]
struct InitializeResult {
    protocol_version: u32,
    agent_info: AgentInfo,
}

#[derive(Serialize)]
struct AgentInfo {
    name: String,
    version: String,
}

// ── Public entry points ──────────────────────────────────────────────

/// Run the agent server over stdio + optional TCP listener.
pub async fn run_agent_server() -> anyhow::Result<()> {
    info!("agent server starting (stdio mode)");
    let transport: Arc<dyn Transport> = Arc::new(StdioTransport::from_std());
    let connection = Arc::new(Connection::new(transport));
    let incoming_rx = connection.start();
    let hub = Arc::new(SessionHub::new());

    let listener = tcp_accept::start_tcp_listener().await;

    let result = if let Some(listener) = listener {
        let hub2 = hub.clone();
        tokio::select! {
            r = run_connection(connection, incoming_rx, &hub) => r,
            r = tcp_accept::accept_loop(listener, hub2) => r,
        }
    } else {
        run_connection(connection, incoming_rx, &hub).await
    };

    server_info::remove_server_info();
    result
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

// ── Connection lifecycle ─────────────────────────────────────────────

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
/// When a session ends, loops back to accept a new agent/start.
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
                    let mut session_handle = crate::session_start::start_session(
                        &connection,
                        id,
                        params,
                        hub,
                        is_production,
                    )
                    .await?;
                    let mut forward_result = crate::session_forward::forward_loop(
                        &mut incoming_rx,
                        &connection,
                        &mut session_handle,
                    )
                    .await;
                    hub.remove_session(&session_handle.session_id).await;
                    // Handle chained agent/start (cancel + new session)
                    while let crate::session_forward::ForwardResult::NewStart {
                        id: new_id,
                        params: new_params,
                    } = forward_result
                    {
                        info!("chained agent/start after session end");
                        session_handle = crate::session_start::start_session(
                            &connection,
                            new_id,
                            new_params,
                            hub,
                            is_production,
                        )
                        .await?;
                        forward_result = crate::session_forward::forward_loop(
                            &mut incoming_rx,
                            &connection,
                            &mut session_handle,
                        )
                        .await;
                        hub.remove_session(&session_handle.session_id).await;
                    }
                    info!("session ended, ready for next");
                    // Loop back to idle — accept new agent/start
                } else if method == methods::AGENT_JOIN.name {
                    // Join the first active session as observer
                    let session_ids = hub.list_session_ids().await;
                    if let Some(sid) = session_ids.first() {
                        if let Some(session) = hub.find_session(sid).await {
                            let client_id = format!("tcp-{id}");
                            session
                                .add_client(client_id.clone(), connection.clone())
                                .await;
                            let _ = connection
                                .respond(id, serde_json::json!({"ok": true, "session_id": sid}))
                                .await;
                            // Observer loop: just forward messages to session, receive events via broadcast
                            crate::session_forward::observer_loop(
                                &mut incoming_rx,
                                &connection,
                                &session,
                                &client_id,
                            )
                            .await;
                            break;
                        }
                    }
                    let _ = connection
                        .respond_error(id, jsonrpc::INVALID_REQUEST, "no active session")
                        .await;
                } else if method == methods::AGENT_SHUTDOWN.name {
                    let _ = connection
                        .respond(id, serde_json::json!({"ok": true}))
                        .await;
                    break;
                } else {
                    let _ = connection
                        .respond_error(id, jsonrpc::METHOD_NOT_FOUND, "expected agent/start")
                        .await;
                }
            }
            Incoming::Notification { .. } => {}
        }
    }
    info!("server shutting down");
    Ok(())
}

pub(crate) async fn wait_for_initialize_with_token(
    connection: &Arc<Connection>,
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    expected_token: Option<&str>,
) -> anyhow::Result<()> {
    loop {
        let Some(msg) = rx.recv().await else {
            anyhow::bail!("connection closed before initialize");
        };
        if let Incoming::Request { id, method, params } = msg {
            if method == methods::INITIALIZE.name {
                if let Some(token) = expected_token {
                    let client_token = params.get("token").and_then(|v| v.as_str());
                    if client_token != Some(token) {
                        let _ = connection
                            .respond_error(id, jsonrpc::INVALID_REQUEST, "invalid token")
                            .await;
                        anyhow::bail!("invalid token");
                    }
                }
                let result = InitializeResult {
                    protocol_version: 1,
                    agent_info: AgentInfo {
                        name: "loopal".into(),
                        version: env!("CARGO_PKG_VERSION").into(),
                    },
                };
                let _ = connection.respond(id, serde_json::to_value(result)?).await;
                info!("IPC initialized");
                return Ok(());
            }
            let _ = connection
                .respond_error(id, jsonrpc::INVALID_REQUEST, "expected initialize first")
                .await;
        }
    }
}

pub(crate) async fn wait_for_initialize(
    connection: &Arc<Connection>,
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
) -> anyhow::Result<()> {
    wait_for_initialize_with_token(connection, rx, None).await
}
