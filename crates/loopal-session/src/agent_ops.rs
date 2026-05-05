use loopal_protocol::UserContent;

use crate::controller::SessionController;
use crate::controller_ops::ControlBackend;
use crate::state::ROOT_AGENT;

impl SessionController {
    /// Send a user message — routes to the active view's agent.
    pub async fn route_message(&self, content: UserContent) {
        let target = self.lock().active_view.clone();
        self.backend.route_to_agent(&target, content).await;
    }

    /// List all agents with their connection state labels. Queries the
    /// Hub via `hub/list_agents` IPC. Returns empty for non-Hub backends.
    pub async fn list_agents(&self) -> Vec<(String, &'static str)> {
        let ControlBackend::Hub(client) = self.backend.as_ref() else {
            return Vec::new();
        };
        let Ok(resp) = client.list_agents().await else {
            return Vec::new();
        };
        resp.get("agents")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        let name = v.get("name")?.as_str()?.to_string();
                        let state = match v.get("state")?.as_str()? {
                            "local" => "local",
                            "connected" => "connected",
                            "shadow" => "shadow",
                            _ => "unknown",
                        };
                        Some((name, state))
                    })
                    .collect()
            })
            .unwrap_or_default()
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
