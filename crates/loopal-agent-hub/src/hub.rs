//! AgentHub — central connection manager and message router.
//!
//! Manages all agent connections uniformly via IPC Connections.
//! Provides message routing (point-to-point, broadcast) and pub/sub channels.
//! Designed to run as an independent process — all agents connect via TCP.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use loopal_ipc::connection::Connection;
use loopal_protocol::{AgentEvent, Envelope};

use crate::routing;
use crate::topology::{AgentInfo, AgentLifecycle};
use crate::types::{AgentConnectionState, LocalChannels, ManagedAgent};

/// Central hub managing connections, message routing, and agent lifecycle.
pub struct AgentHub {
    pub(crate) agents: HashMap<String, ManagedAgent>,
    pub(crate) event_tx: mpsc::Sender<AgentEvent>,
    /// Completion watchers: agent_name → sender.
    pub(crate) completions: HashMap<String, watch::Sender<Option<String>>>,
    /// Cached outputs of finished agents (for late wait_agent calls).
    pub(crate) finished_outputs: HashMap<String, String>,
}

impl AgentHub {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            agents: HashMap::new(),
            event_tx,
            completions: HashMap::new(),
            finished_outputs: HashMap::new(),
        }
    }

    /// Create a no-op hub (for tests).
    pub fn noop() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self {
            agents: HashMap::new(),
            event_tx: tx,
            completions: HashMap::new(),
            finished_outputs: HashMap::new(),
        }
    }

    /// Get a clone of the event sender (for forwarding agent events).
    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    /// Register local channels (for tests — no real TCP connection).
    pub fn set_local(&mut self, name: &str, channels: LocalChannels) {
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Local(channels),
                info: AgentInfo::new(name, None, None),
            },
        );
    }

    /// Register a connected agent.
    /// `parent` is the spawning agent's name (None for root/TUI).
    pub fn register_connection(&mut self, name: &str, conn: Arc<Connection>) -> Result<(), String> {
        self.register_connection_with_parent(name, conn, None, None)
    }

    /// Register a connected agent with parent relationship and model info.
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
        // Record parent→child relationship
        if let Some(p) = parent {
            if let Some(parent_agent) = self.agents.get_mut(p) {
                parent_agent.info.children.push(name.to_string());
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

    /// Unregister an agent: remove from agents map, clean parent's children list,
    /// remove cached output and pending watchers.
    pub fn unregister_connection(&mut self, name: &str) {
        // Extract parent name before mutating
        let parent_name = self.agents.get(name).and_then(|a| a.info.parent.clone());
        if let Some(ref p) = parent_name {
            if let Some(parent) = self.agents.get_mut(p.as_str()) {
                parent.info.children.retain(|c| c != name);
            }
        }
        self.agents.remove(name);
        self.completions.remove(name);
        // Note: finished_outputs NOT removed — kept for late wait_agent callers.
        // Bounded by max agent count per session (typically < 50).
    }

    /// Route an envelope to a named agent via its IPC Connection.
    pub async fn route_message(&self, envelope: &Envelope) -> Result<(), String> {
        let conn = self
            .get_agent_connection(&envelope.target)
            .ok_or_else(|| format!("no agent registered: '{}'", envelope.target))?;
        routing::route_to_agent(&conn, envelope, &self.event_tx).await
    }

    /// Get a named agent's IPC Connection (if connected).
    pub fn get_agent_connection(&self, name: &str) -> Option<Arc<Connection>> {
        self.agents.get(name).and_then(|a| a.state.connection())
    }

    /// Collect all connected agents with their Connections.
    pub fn all_agent_connections(&self) -> Vec<(String, Arc<Connection>)> {
        self.agents
            .iter()
            .filter_map(|(name, agent)| agent.state.connection().map(|c| (name.clone(), c)))
            .collect()
    }

    // ── Topology queries ────────────────────────────────────────────

    /// Get agent info (lifecycle, parent, children, model).
    pub fn agent_info(&self, name: &str) -> Option<&AgentInfo> {
        self.agents.get(name).map(|a| &a.info)
    }

    /// Update an agent's lifecycle state.
    pub fn set_lifecycle(&mut self, name: &str, lifecycle: AgentLifecycle) {
        if let Some(agent) = self.agents.get_mut(name) {
            agent.info.lifecycle = lifecycle;
        }
    }

    /// Get all descendant names of an agent (for cascade shutdown).
    pub fn descendants(&self, name: &str) -> Vec<String> {
        self.agents
            .get(name)
            .map(|a| a.info.descendants(&self.agents))
            .unwrap_or_default()
    }

    /// Build serializable topology snapshot.
    pub fn topology_snapshot(&self) -> serde_json::Value {
        let agents: Vec<serde_json::Value> = self
            .agents
            .iter()
            .filter(|(n, _)| !n.starts_with('_')) // skip internal (_tui)
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
