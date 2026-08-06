use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::rpc_error::RpcError;
use loopal_protocol::{ControlCommand, Envelope, MessageSource, ROOT_AGENT_NAME, UserContent};
use serde_json::Value;
use tokio::sync::oneshot;
use tracing::warn;

#[path = "hub_ui_client/transport_lease.rs"]
mod transport_lease;
use transport_lease::TransportLeaseGuard;

#[cfg(not(test))]
const INTERRUPT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(test)]
const INTERRUPT_DEADLINE: std::time::Duration = std::time::Duration::from_millis(50);

pub struct HubClient {
    conn: Arc<Connection<Listening>>,
    _ui_lease: Option<UiLeaseGuard>,
    _transport_lease: Option<TransportLeaseGuard>,
}

#[cfg(test)]
#[path = "hub_ui_client/tests.rs"]
mod tests;

pub(crate) struct UiLeaseGuard {
    shutdown: Option<oneshot::Sender<()>>,
}

impl UiLeaseGuard {
    pub(crate) fn new(shutdown: oneshot::Sender<()>) -> Self {
        Self {
            shutdown: Some(shutdown),
        }
    }
}

impl Drop for UiLeaseGuard {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

impl HubClient {
    pub fn new(conn: Arc<Connection<Listening>>) -> Self {
        Self {
            conn,
            _ui_lease: None,
            _transport_lease: None,
        }
    }

    /// Own a remote UI transport and close it when the final client is dropped.
    /// In-process `UiSession` uses its separate lease guard instead.
    pub fn new_with_transport_lease(conn: Arc<Connection<Listening>>) -> Self {
        Self {
            _transport_lease: Some(TransportLeaseGuard::new(conn.clone())),
            conn,
            _ui_lease: None,
        }
    }

    pub(crate) fn new_with_ui_lease(
        conn: Arc<Connection<Listening>>,
        shutdown: oneshot::Sender<()>,
    ) -> Self {
        Self {
            conn,
            _ui_lease: Some(UiLeaseGuard::new(shutdown)),
            _transport_lease: None,
        }
    }

    pub async fn send_message(&self, content: UserContent) {
        self.send_message_to(ROOT_AGENT_NAME, content).await;
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

    pub async fn route_envelope(&self, envelope: &Envelope) -> Result<Value, RpcError> {
        self.conn
            .send_request(
                methods::HUB_ROUTE.name,
                serde_json::to_value(envelope).unwrap_or_default(),
            )
            .await
    }

    pub async fn send_control(&self, cmd: &ControlCommand) -> Result<Value, RpcError> {
        self.send_control_to(ROOT_AGENT_NAME, cmd).await
    }

    pub async fn send_control_to(
        &self,
        target: &str,
        cmd: &ControlCommand,
    ) -> Result<Value, RpcError> {
        let params = serde_json::json!({
            "target": target,
            "command": serde_json::to_value(cmd).unwrap_or_default(),
        });
        self.conn
            .send_request(methods::HUB_CONTROL.name, params)
            .await
    }

    pub async fn interrupt(&self) {
        self.interrupt_target(ROOT_AGENT_NAME).await;
    }

    pub async fn interrupt_target(&self, target: &str) {
        let result = tokio::time::timeout(
            INTERRUPT_DEADLINE,
            self.conn.send_request(
                methods::HUB_INTERRUPT.name,
                serde_json::json!({"target": target}),
            ),
        )
        .await;
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => warn!(target, %error, "hub interrupt failed"),
            Err(_) => warn!(target, "hub interrupt timed out"),
        }
    }

    pub async fn list_agents(&self) -> Result<Value, RpcError> {
        self.conn
            .send_request(methods::HUB_LIST_AGENTS.name, serde_json::json!({}))
            .await
    }

    pub async fn shutdown_agent(&self) {
        if let Err(e) = self
            .conn
            .send_request(
                methods::HUB_SHUTDOWN_AGENT.name,
                serde_json::json!({"target": ROOT_AGENT_NAME}),
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

    pub fn connection(&self) -> &Arc<Connection<Listening>> {
        &self.conn
    }
}
