//! Read-side methods for `AgentRegistry`: counts, lookups, listings,
//! routing, and topology snapshot. Extracted from `mod.rs` to keep each
//! file under the 200-line limit.

use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_protocol::Envelope;

use crate::routing;
use crate::topology::{AgentInfo, AgentLifecycle};
use crate::types::AgentConnectionState;

use super::AgentRegistry;

impl AgentRegistry {
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Count only sub-agents (those with a parent). Excludes root "main".
    pub fn sub_agent_count(&self) -> usize {
        self.agents
            .values()
            .filter(|a| a.info.parent.is_some())
            .count()
    }

    pub fn get_agent_connection(&self, name: &str) -> Option<Arc<Connection>> {
        self.agents.get(name).and_then(|a| a.state.connection())
    }

    pub fn all_agent_connections(&self) -> Vec<(String, Arc<Connection>)> {
        self.agents
            .iter()
            .filter_map(|(n, a)| a.state.connection().map(|c| (n.clone(), c)))
            .collect()
    }

    pub fn list_agents(&self) -> Vec<(String, &'static str)> {
        self.agents
            .iter()
            .map(|(n, a)| {
                let l = match &a.state {
                    AgentConnectionState::Local(_) => "local",
                    AgentConnectionState::Connected(_) => "connected",
                    AgentConnectionState::Shadow => "shadow",
                };
                (n.clone(), l)
            })
            .collect()
    }

    pub async fn route_message(&self, envelope: &Envelope) -> Result<(), String> {
        let conn = self
            .get_agent_connection(&envelope.target.agent)
            .ok_or_else(|| format!("no agent: '{}'", envelope.target))?;
        routing::route_to_agent(&conn, envelope, &self.event_tx).await
    }

    pub fn agent_info(&self, name: &str) -> Option<&AgentInfo> {
        self.agents.get(name).map(|a| &a.info)
    }

    pub fn set_lifecycle(&mut self, name: &str, lifecycle: AgentLifecycle) {
        if let Some(a) = self.agents.get_mut(name) {
            a.info.lifecycle = lifecycle;
        }
    }

    pub fn descendants(&self, name: &str) -> Vec<String> {
        self.agents
            .get(name)
            .map(|a| a.info.descendants(&self.agents))
            .unwrap_or_default()
    }

    pub fn topology_snapshot(&self) -> serde_json::Value {
        let agents: Vec<serde_json::Value> = self
            .agents
            .iter()
            .map(|(name, a)| {
                serde_json::json!({
                    "name": name,
                    "parent": a.info.parent.as_ref().map(|p| p.to_string()),
                    "children": a.info.children,
                    "lifecycle": format!("{:?}", a.info.lifecycle),
                    "model": a.info.model,
                })
            })
            .collect();
        serde_json::json!({ "agents": agents })
    }
}
