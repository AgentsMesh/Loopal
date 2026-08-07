//! Read-side methods for `AgentRegistry`: counts, lookups, listings,
//! routing, and topology snapshot. Extracted from `mod.rs` to keep each
//! file under the 200-line limit.

use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentCompletion, Envelope};
use loopal_view_state::ViewStateReducer;
use tokio::sync::Mutex;

use crate::topology::{AgentInfo, AgentLifecycle};
use crate::types::AgentConnectionState;

use super::AgentRegistry;

impl AgentRegistry {
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn managed_agent_count(&self) -> usize {
        self.agents
            .values()
            .filter(|agent| !agent.state.is_shadow())
            .count()
    }

    /// Count only sub-agents (those with a parent). Excludes root "main".
    pub fn sub_agent_count(&self) -> usize {
        self.agents
            .values()
            .filter(|a| a.info.parent.is_some())
            .count()
    }

    pub fn get_agent_connection(&self, name: &str) -> Option<Arc<Connection<Listening>>> {
        self.agents.get(name).and_then(|a| a.state.connection())
    }

    pub(crate) fn is_current_connection(
        &self,
        name: &str,
        expected: &Arc<Connection<Listening>>,
    ) -> bool {
        self.get_agent_connection(name)
            .is_some_and(|current| Arc::ptr_eq(&current, expected))
    }

    /// Per-agent ViewState reducer handle. Used by the hub event router
    /// to apply incoming events and by `view/snapshot` to read state.
    pub fn agent_view(&self, name: &str) -> Option<Arc<Mutex<ViewStateReducer>>> {
        self.agents
            .get(name)
            .map(|agent| agent.view.clone())
            .or_else(|| self.completed.get(name).map(|agent| agent.view.clone()))
    }

    pub fn all_agent_connections(&self) -> Vec<(String, Arc<Connection<Listening>>)> {
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

    /// Clone the routing handles needed for delivery while the registry is
    /// borrowed. Callers must perform network I/O after releasing the Hub lock.
    pub fn route_target(&self, envelope: &Envelope) -> Result<Arc<Connection<Listening>>, String> {
        self.get_agent_connection(&envelope.target.agent)
            .ok_or_else(|| format!("no agent: '{}'", envelope.target))
    }

    pub fn agent_info(&self, name: &str) -> Option<&AgentInfo> {
        self.agents
            .get(name)
            .map(|agent| &agent.info)
            .or_else(|| self.completed.get(name).map(|agent| &agent.info))
    }

    pub fn completion_output(&self, name: &str) -> Option<&str> {
        self.completion(name).map(AgentCompletion::output)
    }

    pub fn completion(&self, name: &str) -> Option<&AgentCompletion> {
        self.agents
            .get(name)
            .and_then(|agent| agent.completion.as_ref())
            .or_else(|| self.completed.get(name).map(|agent| &agent.completion))
    }

    pub fn set_lifecycle(&mut self, name: &str, lifecycle: AgentLifecycle) {
        if let Some(a) = self.agents.get_mut(name) {
            if a.completion.is_some() {
                return;
            }
            a.info.lifecycle = lifecycle;
        } else if self.completed.contains_key(name) {
            // Every completed entry has an authoritative typed completion.
            let _ = lifecycle;
            tracing::debug!(agent = %name, "ignoring lifecycle event for completed generation");
        }
    }

    pub(crate) fn set_completion_lifecycle(&mut self, name: &str, lifecycle: AgentLifecycle) {
        if let Some(a) = self.agents.get_mut(name) {
            a.info.lifecycle = lifecycle;
        } else if let Some(a) = self.completed.get_mut(name) {
            a.info.lifecycle = lifecycle;
        }
    }

    pub fn descendants(&self, name: &str) -> Vec<String> {
        let mut descendants = Vec::new();
        let Some(root_generation) = self.generation(name) else {
            return descendants;
        };
        let mut pending = vec![(name.to_string(), root_generation)];
        while let Some((parent, parent_generation)) = pending.pop() {
            let children = self
                .agent_info(&parent)
                .map(|info| info.children.clone())
                .unwrap_or_default();
            for child in children {
                if self.parent_generation(&child) != Some(parent_generation) {
                    continue;
                }
                if let Some(child_generation) = self.generation(&child) {
                    pending.push((child.clone(), child_generation));
                }
                descendants.push(child);
            }
        }
        descendants
    }

    fn parent_generation(&self, name: &str) -> Option<u64> {
        self.agents
            .get(name)
            .and_then(|agent| agent.parent_generation)
            .or_else(|| {
                self.completed
                    .get(name)
                    .and_then(|agent| agent.parent_generation)
            })
    }

    pub fn topology_snapshot(&self) -> serde_json::Value {
        let mut agents: Vec<serde_json::Value> = self
            .agents
            .iter()
            .map(|(name, agent)| (name, &agent.info, agent.state.is_shadow()))
            .chain(
                self.completed
                    .iter()
                    .map(|(name, agent)| (name, &agent.info, agent.shadow)),
            )
            .map(topology_entry)
            .collect();
        agents.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        serde_json::json!({ "agents": agents })
    }
}

fn topology_entry((name, info, shadow): (&String, &AgentInfo, bool)) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "parent": info.parent.as_ref().map(|p| p.to_string()),
        "children": info.children,
        "lifecycle": info.lifecycle.state(),
        "error": info.lifecycle.error(),
        "model": info.model,
        "shadow": shadow,
    })
}
