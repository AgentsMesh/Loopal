use serde::{Deserialize, Serialize};

use crate::address::QualifiedAddress;
use crate::event_id::{TurnContext, next_event_id};
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
    /// Per-agent ViewState rev after Hub-side apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<u64>,
    /// Hub-local registration generation used to reject queued events after a
    /// same-name reconnect. This routing metadata never crosses IPC.
    #[serde(skip)]
    pub routing_generation: Option<u64>,
    pub payload: AgentEventPayload,
}

impl AgentEvent {
    pub fn root(payload: AgentEventPayload) -> Self {
        Self::for_agent(None, payload)
    }

    pub fn named(name: impl Into<QualifiedAddress>, payload: AgentEventPayload) -> Self {
        Self::for_agent(Some(name.into()), payload)
    }

    /// Envelope reads turn/correlation via `TurnContext::current_or_default`
    /// — explicit opt-in to "outside turn = zero". Producers that emit
    /// inside `scope_turn` get the propagated context; bootstrap / cold
    /// path callers get zeros as documented by `current_or_default`.
    pub fn for_agent(agent_name: Option<QualifiedAddress>, payload: AgentEventPayload) -> Self {
        let ctx = TurnContext::current_or_default();
        Self {
            agent_name,
            event_id: next_event_id(),
            turn_id: ctx.turn_id,
            correlation_id: ctx.correlation_id,
            rev: None,
            routing_generation: None,
            payload,
        }
    }

    /// Like `for_agent`, but `debug_assert!`s that a turn scope is active.
    /// Use from emit paths inside the agent loop hot path that MUST be
    /// scoped; pure boundary emitters (`Started` before first turn, etc.)
    /// stick with `for_agent`.
    pub fn for_agent_in_turn(
        agent_name: Option<QualifiedAddress>,
        payload: AgentEventPayload,
    ) -> Self {
        let ctx = TurnContext::require_current();
        Self {
            agent_name,
            event_id: next_event_id(),
            turn_id: ctx.turn_id,
            correlation_id: ctx.correlation_id,
            rev: None,
            routing_generation: None,
            payload,
        }
    }
}
