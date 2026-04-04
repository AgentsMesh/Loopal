use serde::{Deserialize, Serialize};

use crate::event_id::{current_correlation_id, current_turn_id, next_event_id};
use crate::event_payload::AgentEventPayload;

/// Complete event with agent identity and causality tracking,
/// transported via event channel to consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    /// Agent that produced this event. Hub fills this in for all agents
    /// (root = `Some("main")`, sub-agent = `Some("name")`).
    /// `None` only in the agent process before Hub injection.
    pub agent_name: Option<String>,
    /// Monotonically increasing process-unique ID (0 = unset).
    #[serde(default)]
    pub event_id: u64,
    /// Turn that produced this event (0 = outside a turn).
    #[serde(default)]
    pub turn_id: u32,
    /// Groups related events (e.g. parallel tool batch). 0 = ungrouped.
    #[serde(default)]
    pub correlation_id: u64,
    pub payload: AgentEventPayload,
}

impl AgentEvent {
    /// Convenience: create a root-agent event.
    pub fn root(payload: AgentEventPayload) -> Self {
        Self {
            agent_name: None,
            event_id: next_event_id(),
            turn_id: current_turn_id(),
            correlation_id: current_correlation_id(),
            payload,
        }
    }

    /// Convenience: create a named sub-agent event.
    pub fn named(name: impl Into<String>, payload: AgentEventPayload) -> Self {
        Self {
            agent_name: Some(name.into()),
            event_id: next_event_id(),
            turn_id: current_turn_id(),
            correlation_id: current_correlation_id(),
            payload,
        }
    }
}
