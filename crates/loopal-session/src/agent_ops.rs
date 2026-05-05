//! Agent operations on SessionController: message routing, connection management.

use loopal_protocol::UserContent;

use crate::controller::SessionController;
use crate::state::ROOT_AGENT;

impl SessionController {
    /// Send a user message — routes to the active view's agent.
    pub async fn route_message(&self, content: UserContent) {
        let target = self.lock().active_view.clone();
        self.backend.route_to_agent(&target, content).await;
    }

    /// List all agents with their connection state labels.
    pub async fn list_agents(&self) -> Vec<(String, &'static str)> {
        self.connections().lock().await.registry.list_agents()
    }

    /// Switch the active view to `name`. Returns `false` if `name` is
    /// already the active view. The caller is responsible for filtering
    /// out non-live agents (use `App::is_agent_live`).
    pub fn enter_agent_view(&self, name: &str) -> bool {
        let mut state = self.lock();
        if name == state.active_view {
            return false;
        }
        state.active_view = name.to_string();
        true
    }

    /// Return to root view.
    pub fn exit_agent_view(&self) {
        let mut state = self.lock();
        if state.active_view != ROOT_AGENT {
            state.active_view = ROOT_AGENT.to_string();
        }
    }
}
