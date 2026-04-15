//! AgentEvent → SessionState update logic. Unified routing for all agents.

use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::message_log::record_message_routed;
use crate::state::{ROOT_AGENT, SessionState};

/// Handle an AgentEvent. Updates display state only — messages are delivered
/// directly to the agent mailbox.
pub fn apply_event(state: &mut SessionState, event: AgentEvent) {
    // Global logging for inter-agent messages
    if let AgentEventPayload::MessageRouted {
        ref source,
        ref target,
        ref content_preview,
    } = event.payload
    {
        record_message_routed(state, source, target, content_preview);
    }

    // Topology registration for spawned agents
    if let AgentEventPayload::SubAgentSpawned {
        ref name,
        ref parent,
        ref model,
        ref session_id,
        ..
    } = event.payload
    {
        crate::agent_lifecycle::register_spawned_agent(
            state,
            name,
            parent.as_deref(),
            model.as_deref(),
            session_id.as_deref(),
        );
        // Enqueue for persistence if session_id is known
        if let Some(sid) = session_id {
            state
                .pending_sub_agent_refs
                .push(crate::state::PendingSubAgentRef {
                    name: name.clone(),
                    session_id: sid.clone(),
                    parent: parent.clone(),
                    model: model.clone(),
                });
        }
    }

    // Session-level mode sync: when the active agent's mode changes, update state.mode
    if let AgentEventPayload::ModeChanged { ref mode } = event.payload {
        let agent_name = event.agent_name.as_deref().unwrap_or(ROOT_AGENT);
        if agent_name == state.active_view && state.mode != *mode {
            state.mode.clone_from(mode);
        }
    }

    // Track new root session ID on resume
    if let AgentEventPayload::SessionResumed { ref session_id, .. } = event.payload {
        state.root_session_id = Some(session_id.clone());
    }

    // Cache MCP server status for display
    if let AgentEventPayload::McpStatusReport { ref servers } = event.payload {
        state.mcp_status = Some(servers.clone());
    }

    // Unified: route to agent conversation by name (root = "main")
    let name = event.agent_name.unwrap_or_else(|| ROOT_AGENT.into());
    crate::agent_handler::apply_agent_event(state, &name, event.payload);
}
