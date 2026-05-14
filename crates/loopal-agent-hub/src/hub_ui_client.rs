use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{ControlCommand, Envelope, MessageSource, UserContent};
use serde_json::Value;
use tracing::warn;

pub struct HubClient {
    conn: Arc<Connection>,
}

impl HubClient {
    pub fn new(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    pub async fn send_message(&self, content: UserContent) {
        self.send_message_to("main", content).await;
    }

    pub async fn send_message_to(&self, target: &str, content: UserContent) {
        let envelope = Envelope::new(MessageSource::Human, target, content);
        if let Ok(params) = serde_json::to_value(&envelope) {
            let _ = self
                .conn
                .send_request(methods::HUB_ROUTE.name, params)
                .await;
        }
    }

    pub async fn route_envelope(&self, envelope: &Envelope) -> Result<Value, String> {
        self.conn
            .send_request(
                methods::HUB_ROUTE.name,
                serde_json::to_value(envelope).unwrap_or_default(),
            )
            .await
    }

    pub async fn send_control(&self, cmd: &ControlCommand) -> Result<Value, String> {
        self.send_control_to("main", cmd).await
    }

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

    pub async fn interrupt(&self) {
        self.interrupt_target("main").await;
    }

    pub async fn interrupt_target(&self, target: &str) {
        let _ = self
            .conn
            .send_request(
                methods::HUB_INTERRUPT.name,
                serde_json::json!({"target": target}),
            )
            .await;
    }

    pub async fn list_agents(&self) -> Result<Value, String> {
        self.conn
            .send_request(methods::HUB_LIST_AGENTS.name, serde_json::json!({}))
            .await
    }

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

    pub async fn shutdown_hub(&self) {
        if let Err(e) = self
            .conn
            .send_request(methods::HUB_SHUTDOWN.name, serde_json::json!({}))
            .await
        {
            warn!("failed to send hub/shutdown: {e}");
        }
    }

    pub fn connection(&self) -> &Arc<Connection> {
        &self.conn
    }
}
