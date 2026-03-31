//! IPC handshake — `initialize` message handling.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::jsonrpc;
use loopal_ipc::protocol::methods;

#[derive(Deserialize)]
struct _InitializeParams {
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
