//! Agent operations on SessionController: message routing, connection management.

use loopal_protocol::{Envelope, MessageSource, UserContent};

use crate::controller::SessionController;

impl SessionController {
    /// Send a user message to the root agent.
    pub async fn route_message(&self, content: UserContent) {
        let primary = self.primary();
        if let Some(ref tx) = primary.mailbox_tx {
            let envelope = Envelope::new(MessageSource::Human, "main", content);
            if let Err(e) = tx.send(envelope).await {
                tracing::warn!(error = %e, "failed to route human message");
            }
        } else {
            tracing::warn!("no mailbox_tx configured — message dropped");
        }
    }

    /// List all agents with their connection state labels.
    pub async fn list_agents(&self) -> Vec<(String, &'static str)> {
        self.connections().lock().await.list_agents()
    }

    /// Detach from a sub-agent (agent keeps running).
    pub async fn detach_agent(&self, name: &str) {
        self.connections().lock().await.detach(name);
    }

    /// Re-attach to a previously detached sub-agent.
    pub async fn reattach_agent(&self, name: &str) -> anyhow::Result<()> {
        self.connections().lock().await.reattach(name).await
    }
}
