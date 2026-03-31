//! Hub UI client — unified interface for UI clients to communicate with Hub.
//!
//! Encapsulates all `hub/*` IPC operations. Both SessionController
//! and ACP (via AcpAdapter) use this instead of calling `send_request` directly.

use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{ControlCommand, Envelope, MessageSource, UserContent, UserQuestionResponse};
use serde_json::Value;
use tracing::warn;

/// Client handle for UI clients to communicate with the Hub.
///
/// Wraps a Hub `Connection` and provides typed methods for all `hub/*` operations.
/// All Hub protocol knowledge (method names, parameter shapes) is centralized here.
pub struct HubClient {
    conn: Arc<Connection>,
}

impl HubClient {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    // ── Message routing ──────────────────────────────────────────────

    /// Send a user message to the root agent via Hub.
    pub async fn send_message(&self, content: UserContent) {
        self.send_message_to("main", content).await;
    }

    /// Send a user message to a specific named agent.
    pub async fn send_message_to(&self, target: &str, content: UserContent) {
        let envelope = Envelope::new(MessageSource::Human, target, content);
        if let Ok(params) = serde_json::to_value(&envelope) {
            let _ = self
                .conn
                .send_request(methods::HUB_ROUTE.name, params)
                .await;
        }
    }

    /// Send a pre-built envelope to a named agent via Hub.
    pub async fn route_envelope(&self, envelope: &Envelope) -> Result<Value, String> {
        self.conn
            .send_request(
                methods::HUB_ROUTE.name,
                serde_json::to_value(envelope).unwrap_or_default(),
            )
            .await
    }

    // ── Control commands ─────────────────────────────────────────────

    /// Send a control command to the root agent.
    pub async fn send_control(&self, cmd: &ControlCommand) -> Result<Value, String> {
        self.send_control_to("main", cmd).await
    }

    /// Send a control command to a specific named agent.
    pub async fn send_control_to(
        &self,
        target: &str,
        cmd: &ControlCommand,
    ) -> Result<Value, String> {
        let params = serde_json::json!({
            "target": target,
            "command": serde_json::to_value(cmd).unwrap_or_default(),
        });
        self.conn
            .send_request(methods::HUB_CONTROL.name, params)
            .await
    }

    /// Interrupt the root agent.
    pub async fn interrupt(&self) {
        self.interrupt_target("main").await;
    }

    /// Interrupt a specific named agent.
    pub async fn interrupt_target(&self, target: &str) {
        let _ = self
            .conn
            .send_request(
                methods::HUB_INTERRUPT.name,
                serde_json::json!({"target": target}),
            )
            .await;
    }

    // ── Permission / question response ───────────────────────────────

    /// Respond to a permission relay request from Hub.
    pub async fn respond_permission(&self, request_id: i64, allow: bool) {
        if let Err(e) = self
            .conn
            .respond(request_id, serde_json::json!({"allow": allow}))
            .await
        {
            warn!("failed to send permission response: {e}");
        }
    }

    /// Respond to a question relay request from Hub.
    pub async fn respond_question(&self, request_id: i64, answers: Vec<String>) {
        let resp = UserQuestionResponse { answers };
        if let Err(e) = self
            .conn
            .respond(request_id, serde_json::to_value(resp).unwrap_or_default())
            .await
        {
            warn!("failed to send question response: {e}");
        }
    }

    // ── Queries ──────────────────────────────────────────────────────

    /// List all agents registered in Hub.
    pub async fn list_agents(&self) -> Result<Value, String> {
        self.conn
            .send_request(methods::HUB_LIST_AGENTS.name, serde_json::json!({}))
            .await
    }

    /// Shut down the root agent.
    pub async fn shutdown_agent(&self) {
        if let Err(e) = self
            .conn
            .send_request(
                methods::HUB_SHUTDOWN_AGENT.name,
                serde_json::json!({"target": "main"}),
            )
            .await
        {
            warn!("failed to send shutdown: {e}");
        }
    }

    /// Access the underlying Hub connection (for custom IPC if needed).
    pub fn connection(&self) -> &Arc<Connection> {
        &self.conn
    }
}
