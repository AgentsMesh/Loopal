//! AgentHub — central connection manager for all agents.
//!
//! Manages root agent (Primary via stdio Bridge) and sub-agents
//! (Attached via TCP). Acts as a network hub: all agent events fan-in
//! here, then fan-out to subscribed frontends (TUI, GUI, etc.).

use std::collections::HashMap;

use tokio::sync::mpsc;

use loopal_protocol::AgentEvent;

use crate::types::{AgentConnectionState, ManagedAgent, PrimaryConn};

/// Central hub managing connections to all agent servers.
pub struct AgentHub {
    pub(crate) agents: HashMap<String, ManagedAgent>,
    pub(crate) event_tx: mpsc::Sender<AgentEvent>,
}

impl AgentHub {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            agents: HashMap::new(),
            event_tx,
        }
    }

    /// Create a no-op hub (for tests).
    pub fn noop() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self {
            agents: HashMap::new(),
            event_tx: tx,
        }
    }

    /// Register the root agent (called once at bootstrap).
    pub fn set_primary(&mut self, name: &str, conn: PrimaryConn) {
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Primary(conn),
            },
        );
    }

    /// Get the primary (root) agent connection for sending user input.
    pub fn primary(&self) -> Option<(&str, &PrimaryConn)> {
        self.agents.iter().find_map(|(name, agent)| {
            if let AgentConnectionState::Primary(conn) = &agent.state {
                Some((name.as_str(), conn))
            } else {
                None
            }
        })
    }
}
