use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};

use loopal_protocol::{AgentEvent, AgentEventPayload};

use super::super::permission_handler::{PermissionHandler, PermissionOutcome};

pub struct ManualPermissionHandler {
    event_tx: mpsc::Sender<AgentEvent>,
    permission_rx: Mutex<mpsc::Receiver<bool>>,
}

impl ManualPermissionHandler {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>, permission_rx: mpsc::Receiver<bool>) -> Self {
        Self {
            event_tx,
            permission_rx: Mutex::new(permission_rx),
        }
    }
}

#[async_trait]
impl PermissionHandler for ManualPermissionHandler {
    async fn decide(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionOutcome {
        let event = AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
            id: id.to_string(),
            name: name.to_string(),
            input: input.clone(),
        });
        if self.event_tx.send(event).await.is_err() {
            warn!(tool = name, "permission channel closed, denying tool");
            return PermissionOutcome::deny("permission channel closed");
        }

        let mut rx = self.permission_rx.lock().await;
        match rx.recv().await {
            Some(true) => {
                info!(tool = name, decision = "allow", "permission");
                PermissionOutcome::allow()
            }
            Some(false) => {
                info!(tool = name, decision = "deny", "permission");
                PermissionOutcome::deny("user denied")
            }
            None => {
                info!(tool = name, decision = "deny", "permission rx closed");
                PermissionOutcome::deny("permission rx closed")
            }
        }
    }
}
