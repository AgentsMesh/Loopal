//! IPC handshake — `initialize` message handling.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::jsonrpc;
use loopal_ipc::protocol::methods;

#[derive(Deserialize)]
struct _InitializeParams {
    #[serde(default)]
    #[allow(dead_code)]
    protocol_version: u32,
}

#[derive(Serialize)]
pub(crate) struct InitializeResult {
    pub protocol_version: u32,
    pub agent_info: AgentInfo,
}

#[derive(Serialize)]
pub(crate) struct AgentInfo {
    pub name: String,
    pub version: String,
}

/// Build the canonical `initialize` response.
/// Centralised so the first-call path (`wait_for_initialize_with_token`) and
/// the idempotent re-call path (`dispatch_simple`) return identical results.
pub(crate) fn build_initialize_result() -> InitializeResult {
    InitializeResult {
        protocol_version: 1,
        agent_info: AgentInfo {
            name: "loopal".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
    }
}

pub(crate) async fn wait_for_initialize_with_token(
    connection: &Arc<Connection<Listening>>,
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
                let result = build_initialize_result();
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
    connection: &Arc<Connection<Listening>>,
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
) -> anyhow::Result<()> {
    wait_for_initialize_with_token(connection, rx, None).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loopal_ipc::connection::{Connection, Incoming, Listening};
    use loopal_ipc::protocol::methods;

    use super::wait_for_initialize_with_token;

    fn connection_pair() -> (
        Arc<Connection<Listening>>,
        Arc<Connection<Listening>>,
        tokio::sync::mpsc::Receiver<Incoming>,
    ) {
        let (server_transport, client_transport) = loopal_ipc::duplex_pair();
        let (server, server_rx) = Connection::new(server_transport).into_listening();
        let (client, _client_rx) = Connection::new(client_transport).into_listening();
        (server, client, server_rx)
    }

    #[tokio::test]
    async fn invalid_token_is_rejected_before_initialization() {
        let (server, client, mut server_rx) = connection_pair();
        let server_task = tokio::spawn(async move {
            wait_for_initialize_with_token(&server, &mut server_rx, Some("expected")).await
        });

        let client_result = client
            .send_request(
                methods::INITIALIZE.name,
                serde_json::json!({"token": "wrong"}),
            )
            .await;

        let error = client_result.expect_err("token request must fail");
        assert_eq!(
            error.remote_code(),
            Some(loopal_ipc::jsonrpc::INVALID_REQUEST)
        );
        assert!(server_task.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn non_initialize_requests_and_notifications_are_ignored_until_valid_token() {
        let (server, client, mut server_rx) = connection_pair();
        let server_task = tokio::spawn(async move {
            wait_for_initialize_with_token(&server, &mut server_rx, Some("expected")).await
        });
        client
            .send_notification("agent/noise", serde_json::Value::Null)
            .await
            .unwrap();
        let wrong = client
            .send_request("agent/not-initialize", serde_json::Value::Null)
            .await
            .unwrap_err();
        assert_eq!(
            wrong.remote_code(),
            Some(loopal_ipc::jsonrpc::INVALID_REQUEST)
        );
        let initialized = client
            .send_request(
                methods::INITIALIZE.name,
                serde_json::json!({"token": "expected"}),
            )
            .await
            .unwrap();
        assert_eq!(initialized["protocol_version"], 1);
        assert!(server_task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn closed_input_channel_is_reported_before_initialization() {
        let (transport, _peer) = loopal_ipc::duplex_pair();
        let (connection, _incoming) = Connection::new(transport).into_listening();
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        drop(sender);

        let error = wait_for_initialize_with_token(&connection, &mut receiver, None)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("connection closed before initialize")
        );
    }
}
