//! Agent registry — manages agent connections, lifecycle, routing.
//!
//! Contains only agent-related state. UI client management is in `UiDispatcher`.

mod completion;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use loopal_ipc::connection::Connection;
use loopal_protocol::{AgentEvent, Envelope};

use crate::routing;
use crate::topology::{AgentInfo, AgentLifecycle};
use crate::types::{AgentConnectionState, LocalChannels, ManagedAgent};

/// Pure agent registry — no UI client knowledge.
pub struct AgentRegistry {
    pub(crate) agents: HashMap<String, ManagedAgent>,
    pub(crate) event_tx: mpsc::Sender<AgentEvent>,
    pub(crate) completions: HashMap<String, watch::Sender<Option<String>>>,
    pub(crate) finished_outputs: HashMap<String, String>,
}

impl AgentRegistry {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            agents: HashMap::new(),
            event_tx,
            completions: HashMap::new(),
            finished_outputs: HashMap::new(),
        }
    }

    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    // ── Registration ─────────────────────────────────────────────

    pub fn set_local(&mut self, name: &str, channels: LocalChannels) {
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Local(channels),
                info: AgentInfo::new(name, None, None),
            },
        );
    }

    pub fn register_connection(&mut self, name: &str, conn: Arc<Connection>) -> Result<(), String> {
        self.register_connection_with_parent(name, conn, None, None)
    }

    pub fn register_connection_with_parent(
        &mut self,
        name: &str,
        conn: Arc<Connection>,
        parent: Option<&str>,
        model: Option<&str>,
    ) -> Result<(), String> {
        if self.agents.contains_key(name) {
            return Err(format!("agent '{name}' already registered"));
        }
        if let Some(p) = parent {
            if let Some(pa) = self.agents.get_mut(p) {
                pa.info.children.push(name.to_string());
            }
        }
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Connected(conn),
                info: AgentInfo::new(name, parent, model),
            },
        );
        Ok(())
    }

    pub fn unregister_connection(&mut self, name: &str) {
        let parent = self.agents.get(name).and_then(|a| a.info.parent.clone());
        if let Some(ref p) = parent {
            if let Some(pa) = self.agents.get_mut(p.as_str()) {
                pa.info.children.retain(|c| c != name);
            }
        }
        self.agents.remove(name);
        self.completions.remove(name);
    }

    // ── Queries ──────────────────────────────────────────────────

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
                };
                (n.clone(), l)
            })
            .collect()
    }

    // ── Routing ──────────────────────────────────────────────────

    pub async fn route_message(&self, envelope: &Envelope) -> Result<(), String> {
        let conn = self
            .get_agent_connection(&envelope.target)
            .ok_or_else(|| format!("no agent: '{}'", envelope.target))?;
        routing::route_to_agent(&conn, envelope, &self.event_tx).await
    }

    // ── Topology ─────────────────────────────────────────────────

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
                    "parent": a.info.parent,
                    "children": a.info.children,
                    "lifecycle": format!("{:?}", a.info.lifecycle),
                    "model": a.info.model,
                })
            })
            .collect();
        serde_json::json!({ "agents": agents })
    }
}
