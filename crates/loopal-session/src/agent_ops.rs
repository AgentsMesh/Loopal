//! Agent operations on SessionController: message routing, connection management.

use loopal_protocol::UserContent;

use crate::controller::SessionController;

impl SessionController {
    /// Send a user message to the root agent.
    pub async fn route_message(&self, content: UserContent) {
        self.backend.send_message(content).await;
    }

    /// List all agents with their connection state labels.
    pub async fn list_agents(&self) -> Vec<(String, &'static str)> {
        self.connections().lock().await.registry.list_agents()
    }
}
