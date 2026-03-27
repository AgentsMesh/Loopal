//! ACP adapter — bridges ACP (session/*) with IPC (agent/*) protocol.

use std::sync::Arc;

use serde_json::Value;
use tokio::io::AsyncBufReadExt;
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{Envelope, MessageSource};

use crate::jsonrpc::{self, IncomingMessage, JsonRpcTransport, read_message};
use crate::types::*;

/// Bridges an ACP client (IDE) with an Agent Server (IPC).
pub struct AcpAdapter {
    pub(crate) agent_conn: Arc<Connection>,
    pub(crate) agent_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Incoming>>,
    pub(crate) acp_out: Arc<JsonRpcTransport>,
    pub(crate) session_id: tokio::sync::Mutex<Option<String>>,
}

impl AcpAdapter {
    pub fn new(
        agent_conn: Arc<Connection>,
        agent_rx: tokio::sync::mpsc::Receiver<Incoming>,
        acp_out: Arc<JsonRpcTransport>,
    ) -> Self {
        Self {
            agent_conn,
            agent_rx: tokio::sync::Mutex::new(agent_rx),
            acp_out,
            session_id: tokio::sync::Mutex::new(None),
        }
    }

    /// Run the ACP adapter loop: read IDE requests + forward agent events.
    pub async fn run(
        &self,
        reader: &mut (impl AsyncBufReadExt + Unpin),
    ) -> anyhow::Result<()> {
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
                Some(IncomingMessage::Notification { .. }) => {}
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
            "session/new" => self.handle_new_session(id, params).await,
            "session/prompt" => self.handle_prompt(id, params).await,
            "session/cancel" => self.handle_cancel(id).await,
            _ => {
                self.acp_out
                    .respond_error(id, jsonrpc::METHOD_NOT_FOUND, &format!("unknown: {method}"))
                    .await;
            }
        }
    }

    async fn handle_initialize(&self, id: i64, _params: Value) {
        let result = InitializeResult {
            protocol_version: 1,
            agent_capabilities: AgentCapabilities { streaming: true },
            agent_info: AgentInfo {
                name: "loopal".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        self.acp_out
            .respond(id, serde_json::to_value(result).unwrap_or_default())
            .await;
        info!("ACP initialized");
    }

    async fn handle_new_session(&self, id: i64, params: Value) {
        self.handle_new_session_inner(id, params).await;
    }

    async fn handle_prompt(&self, id: i64, params: Value) {
        let params: PromptParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                self.acp_out
                    .respond_error(id, jsonrpc::INVALID_REQUEST, &e.to_string())
                    .await;
                return;
            }
        };

        let session_id = self.session_id.lock().await.clone();
        let Some(ref sid) = session_id else {
            self.acp_out
                .respond_error(id, jsonrpc::INVALID_REQUEST, "no session")
                .await;
            return;
        };
        if *sid != params.session_id {
            self.acp_out
                .respond_error(id, jsonrpc::INVALID_REQUEST, "session mismatch")
                .await;
            return;
        }

        // Build text from content blocks
        let text: String = params
            .prompt
            .iter()
            .map(|b| match b {
                AcpContentBlock::Text { text } => text.as_str(),
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Send as agent/message to agent server
        let envelope = Envelope::new(MessageSource::Human, "main", text);
        if let Err(e) = self
            .agent_conn
            .send_request(methods::AGENT_MESSAGE.name, serde_json::to_value(&envelope).unwrap())
            .await
        {
            self.acp_out
                .respond_error(id, jsonrpc::INTERNAL_ERROR, &e)
                .await;
            return;
        }

        // Event loop: read agent events, translate, forward to IDE
        let stop_reason = self.run_event_loop(sid).await;
        let result = PromptResult { stop_reason };
        self.acp_out
            .respond(id, serde_json::to_value(result).unwrap_or_default())
            .await;
    }

    async fn handle_cancel(&self, id: i64) {
        if self.session_id.lock().await.is_none() {
            self.acp_out
                .respond_error(id, jsonrpc::INVALID_REQUEST, "no active session")
                .await;
            return;
        }
        let _ = self
            .agent_conn
            .send_notification(methods::AGENT_INTERRUPT.name, serde_json::json!({}))
            .await;
        self.acp_out.respond(id, Value::Null).await;
    }
}
