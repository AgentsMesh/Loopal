//! ACP (Agent Client Protocol) server for IDE integration.
//!
//! Provides a JSON-RPC 2.0 interface over stdin/stdout, activated via `--acp`.
//! Internally spawns an Agent Server child process and bridges the ACP
//! protocol (`session/*`) to the IPC protocol (`agent/*`).

mod adapter;
mod adapter_events;
pub mod jsonrpc;
mod translate;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::info;

use loopal_config::ResolvedConfig;
use loopal_ipc::connection::Connection;
use loopal_ipc::transport::Transport;

pub use crate::adapter::AcpAdapter;
use crate::jsonrpc::JsonRpcTransport;

/// Run Loopal as an ACP server (stdin/stdout JSON-RPC).
///
/// Spawns an Agent Server child process, then bridges the ACP protocol
/// from the IDE with the IPC protocol to the Agent Server.
pub async fn run_acp(_config: ResolvedConfig, _cwd: PathBuf) -> anyhow::Result<()> {
    info!("starting ACP server (bridge mode)");

    let agent_proc = loopal_agent_client::AgentProcess::spawn(None).await?;
    let transport = agent_proc.transport();
    let agent_conn = Arc::new(Connection::new(transport));
    let agent_rx = agent_conn.start();

    // Initialize the agent server
    let init_result = agent_conn
        .send_request("initialize", serde_json::json!({"protocol_version": 1}))
        .await
        .map_err(|e| anyhow::anyhow!("agent initialize failed: {e}"))?;
    info!(?init_result, "agent server initialized");

    let acp_out = Arc::new(JsonRpcTransport::new());
    let adapter = AcpAdapter::new(agent_conn, agent_rx, acp_out);
    let mut reader = BufReader::new(tokio::io::stdin());
    adapter.run(&mut reader).await
}

/// Run ACP with a pre-established agent transport (for testing).
pub async fn run_acp_with_transport(
    agent_transport: Arc<dyn Transport>,
    acp_out: Arc<JsonRpcTransport>,
    reader: &mut (impl AsyncBufReadExt + Unpin),
) -> anyhow::Result<()> {
    let agent_conn = Arc::new(Connection::new(agent_transport));
    let agent_rx = agent_conn.start();

    // Initialize agent server over the transport
    let _ = agent_conn
        .send_request("initialize", serde_json::json!({"protocol_version": 1}))
        .await
        .map_err(|e| anyhow::anyhow!("agent initialize failed: {e}"))?;

    let adapter = AcpAdapter::new(agent_conn, agent_rx, acp_out);
    adapter.run(reader).await
}
