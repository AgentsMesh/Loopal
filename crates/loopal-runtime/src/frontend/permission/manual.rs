use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};

use loopal_protocol::{AgentEvent, AgentEventPayload, PermissionIntentRequest};

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
    async fn decide(&self, request: &PermissionIntentRequest) -> PermissionOutcome {
        let event = AgentEvent::root(AgentEventPayload::ToolPermissionRequest {
            id: request.tool_call_id.clone(),
            name: request.tool_name.clone(),
            input: request.display_input.clone(),
            permission_intent: None,
        });
        if self.event_tx.send(event).await.is_err() {
            warn!(tool = %request.tool_name, "permission channel closed, denying tool");
            return PermissionOutcome::deny("permission channel closed");
        }

        let mut rx = self.permission_rx.lock().await;
        match rx.recv().await {
            Some(true) => {
                info!(tool = %request.tool_name, decision = "allow", "permission");
                PermissionOutcome::allow()
            }
            Some(false) => {
                info!(tool = %request.tool_name, decision = "deny", "permission");
                PermissionOutcome::deny("user denied")
            }
            None => {
                info!(tool = %request.tool_name, decision = "deny", "permission rx closed");
                PermissionOutcome::deny("permission rx closed")
            }
        }
    }
}
