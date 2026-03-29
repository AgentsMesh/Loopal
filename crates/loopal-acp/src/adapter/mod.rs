//! ACP adapter — bridges ACP (session/*) with Hub via UiSession.

mod control;
mod events;
mod lifecycle;
mod permission;
mod prompt;
mod session;

use std::sync::Arc;

use serde_json::Value;
use tokio::io::AsyncBufReadExt;
use tokio::sync::broadcast;
use tracing::info;

use loopal_agent_hub::{HubClient, UiSession};
use loopal_ipc::connection::Incoming;
use loopal_protocol::AgentEvent;

use crate::jsonrpc::{self, IncomingMessage, JsonRpcTransport, read_message};

/// Bridges an ACP client (IDE) with the Hub via `UiSession`.
pub struct AcpAdapter {
    /// Hub communication client.
    pub(crate) client: Arc<HubClient>,
    /// Incoming relay requests from Hub (permission/question).
    pub(crate) relay_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Incoming>>,
    /// Agent events from Hub broadcast.
    pub(crate) event_rx: tokio::sync::Mutex<broadcast::Receiver<AgentEvent>>,
    /// JSON-RPC output to IDE (stdout).
    pub(crate) acp_out: Arc<JsonRpcTransport>,
    /// Active session ID.
    pub(crate) session_id: tokio::sync::Mutex<Option<String>>,
}

impl AcpAdapter {
    /// Create from a UiSession (extracts fields into Mutex wrappers).
    pub fn new(ui: UiSession, acp_out: Arc<JsonRpcTransport>) -> Self {
        Self {
            client: ui.client,
            relay_rx: tokio::sync::Mutex::new(ui.relay_rx),
            event_rx: tokio::sync::Mutex::new(ui.event_rx),
            acp_out,
            session_id: tokio::sync::Mutex::new(None),
        }
    }

    /// Run the ACP adapter loop.
    pub async fn run(&self, reader: &mut (impl AsyncBufReadExt + Unpin)) -> anyhow::Result<()> {
        loop {
            match read_message(reader).await {
                Some(IncomingMessage::Request { id, method, params }) => {
                    self.dispatch(id, &method, params).await;
                }
                Some(IncomingMessage::Response { id, result, error }) => {
                    let value = result.unwrap_or_else(|| {
                        error
                            .map(|e| serde_json::to_value(e).unwrap_or_default())
                            .unwrap_or(Value::Null)
                    });
                    self.acp_out.route_response(id, value).await;
                }
                Some(IncomingMessage::Notification { method, params }) => {
                    self.dispatch_notification(&method, params).await;
                }
                None => {
                    info!("ACP reader closed, shutting down");
                    break;
                }
            }
        }
        Ok(())
    }

    async fn dispatch(&self, id: i64, method: &str, params: Value) {
        match method {
            "initialize" => self.handle_initialize(id, params).await,
            "authenticate" => self.handle_authenticate(id, params).await,
            "session/new" => self.handle_new_session(id, params).await,
            "session/list" => self.handle_list_sessions(id).await,
            "session/prompt" => self.handle_prompt(id, params).await,
            "session/cancel" => self.handle_cancel_request(id).await,
            "session/close" => self.handle_close(id, params).await,
            "session/set_mode" => self.handle_set_mode(id, params).await,
            "session/set_config_option" => self.handle_set_config_option(id, params).await,
            "session/load" => {
                self.acp_out
                    .respond_error(
                        id,
                        jsonrpc::INTERNAL_ERROR,
                        "session/load not supported (use session/new)",
                    )
                    .await;
            }
            _ => {
                self.acp_out
                    .respond_error(id, jsonrpc::METHOD_NOT_FOUND, &format!("unknown: {method}"))
                    .await;
            }
        }
    }

    async fn dispatch_notification(&self, method: &str, _params: Value) {
        if method == "session/cancel" {
            self.handle_cancel_notification().await;
        }
    }
}
