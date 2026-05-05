use serde::{Deserialize, Serialize};

use crate::address::QualifiedAddress;
use crate::event_id::{current_correlation_id, current_turn_id, next_event_id};
use crate::event_payload::AgentEventPayload;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub agent_name: Option<QualifiedAddress>,
    /// 0 = untracked.
    #[serde(default)]
    pub event_id: u64,
    /// 0 = outside a turn.
    #[serde(default)]
    pub turn_id: u32,
    /// 0 = ungrouped.
    #[serde(default)]
    pub correlation_id: u64,
    /// Per-agent ViewState rev after Hub-side apply. UI clients drop
    /// events whose `rev` is at or below the reducer's current `rev`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<u64>,
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
            rev: None,
            payload,
        }
    }

    /// Convenience: create a named sub-agent event.
    /// `name` may be a bare agent name or a qualified `hub/agent` form.
    pub fn named(name: impl Into<QualifiedAddress>, payload: AgentEventPayload) -> Self {
        Self {
            agent_name: Some(name.into()),
            event_id: next_event_id(),
            turn_id: current_turn_id(),
            correlation_id: current_correlation_id(),
            rev: None,
            payload,
        }
    }
}
