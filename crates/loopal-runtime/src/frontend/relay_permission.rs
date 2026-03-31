use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};

use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_tool_api::PermissionDecision;

use super::permission_handler::PermissionHandler;

/// Permission handler that relays requests to an external consumer via event channel.
///
/// Used by `UnifiedFrontend` for root agents where permission decisions are
/// made by an external consumer .
pub struct RelayPermissionHandler {
    event_tx: mpsc::Sender<AgentEvent>,
    permission_rx: Mutex<mpsc::Receiver<bool>>,
}

impl RelayPermissionHandler {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>, permission_rx: mpsc::Receiver<bool>) -> Self {
        Self {
            event_tx,
            permission_rx: Mutex::new(permission_rx),
        }
    }
}

#[async_trait]
impl PermissionHandler for RelayPermissionHandler {
    async fn decide(&self, id: &str, name: &str, input: &serde_json::Value) -> PermissionDecision {
        let event = AgentEvent {
            agent_name: None,
            payload: AgentEventPayload::ToolPermissionRequest {
                id: id.to_string(),
                name: name.to_string(),
                input: input.clone(),
            },
        };
        let send_ok = self.event_tx.send(event).await.is_ok();
        if !send_ok {
            warn!(tool = name, "permission channel closed, denying tool");
            return PermissionDecision::Deny;
        }

        let mut rx = self.permission_rx.lock().await;
        match rx.recv().await {
            Some(true) => {
                info!(tool = name, decision = "allow", "permission");
                PermissionDecision::Allow
            }
            _ => {
                info!(tool = name, decision = "deny", "permission");
                PermissionDecision::Deny
            }
        }
    }
}
