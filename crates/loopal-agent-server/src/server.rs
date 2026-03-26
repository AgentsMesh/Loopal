//! Agent server entry point — handles IPC lifecycle and spawns agent loop.
//!
//! Activated via `loopal --serve`. Listens on stdio for JSON-RPC messages,
//! processes `initialize` and `agent/start`, then runs the agent loop.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::info;

use loopal_config::load_config;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_ipc::{StdioTransport, jsonrpc};
use loopal_runtime::agent_loop;

use crate::params::{self, StartParams};

// ── Types ────────────────────────────────────────────────────────────

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

/// Run the agent server over stdio. Blocks until the connection closes.
pub async fn run_agent_server() -> anyhow::Result<()> {
    info!("agent server starting (stdio mode)");
    let transport: Arc<dyn Transport> = Arc::new(StdioTransport::from_std());
    let connection = Arc::new(Connection::new(transport));
    let incoming_rx = connection.start();
    run_server_loop(connection, incoming_rx).await
}

/// Run the agent server with mock provider loaded from a JSON file (for system tests).
pub async fn run_agent_server_with_mock(mock_path: &str) -> anyhow::Result<()> {
    info!(mock_path, "agent server starting with mock provider");
    let provider = crate::mock_loader::load_mock_provider(mock_path)?;
    let cwd = std::env::current_dir().unwrap_or_default();
    let session_dir = std::env::temp_dir().join("loopal-test-sessions");
    let transport: Arc<dyn Transport> = Arc::new(StdioTransport::from_std());
    crate::test_server::run_server_for_test(transport, provider, cwd, session_dir).await
}

// ── Server loop ──────────────────────────────────────────────────────

async fn run_server_loop(
    connection: Arc<Connection>,
    mut incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
) -> anyhow::Result<()> {
    wait_for_initialize(&connection, &mut incoming_rx).await?;

    loop {
        let Some(msg) = incoming_rx.recv().await else {
            info!("IPC connection closed, shutting down");
            break;
        };
        match msg {
            Incoming::Request { id, method, params } => {
                if method == methods::AGENT_START.name {
                    handle_agent_start(&connection, incoming_rx, id, params).await?;
                    break;
                } else if method == methods::AGENT_SHUTDOWN.name {
                    let _ = connection.respond(id, serde_json::json!({"ok": true})).await;
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
    info!("agent server shutting down");
    Ok(())
}

pub(crate) async fn wait_for_initialize(
    connection: &Arc<Connection>,
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
) -> anyhow::Result<()> {
    loop {
        let Some(msg) = rx.recv().await else {
            anyhow::bail!("connection closed before initialize");
        };
        if let Incoming::Request { id, method, params } = msg {
            if method == methods::INITIALIZE.name {
                let _: InitializeParams = serde_json::from_value(params)
                    .unwrap_or(InitializeParams { protocol_version: 1 });
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

async fn handle_agent_start(
    connection: &Arc<Connection>,
    incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    request_id: i64,
    params: serde_json::Value,
) -> anyhow::Result<()> {
    let cwd_str = params["cwd"].as_str().map(String::from);
    let cwd = cwd_str
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let config = load_config(&cwd)?;
    let start = StartParams {
        cwd: cwd_str,
        model: params["model"].as_str().map(String::from),
        mode: params["mode"].as_str().map(String::from),
        prompt: params["prompt"].as_str().map(String::from),
        permission_mode: params["permission_mode"].as_str().map(String::from),
        no_sandbox: params["no_sandbox"].as_bool().unwrap_or(false),
    };

    let agent_params = params::build(&cwd, &config, &start, connection, incoming_rx).await?;

    let _ = connection
        .respond(request_id, serde_json::json!({"session_id": agent_params.session.id}))
        .await;

    match agent_loop(agent_params).await {
        Ok(output) => info!(reason = ?output.terminate_reason, "agent loop completed"),
        Err(e) => tracing::error!(error = %e, "agent loop error"),
    }
    Ok(())
}
