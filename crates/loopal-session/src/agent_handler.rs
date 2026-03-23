//! Agent event handling — updates AgentViewState tracking.

use std::time::Instant;

use loopal_protocol::AgentEventPayload;
use loopal_protocol::AgentStatus;

use crate::state::SessionState;

/// Handle a sub-agent event by updating its AgentViewState.
pub(crate) fn apply_agent_event(state: &mut SessionState, name: &str, payload: AgentEventPayload) {
    let agent = state.agents.entry(name.to_string()).or_default();

    // Record first-seen timestamp for elapsed time display
    if agent.started_at.is_none() {
        agent.started_at = Some(Instant::now());
    }

    match &payload {
        AgentEventPayload::Started => {
            agent.observable.status = AgentStatus::Running;
        }
        AgentEventPayload::ToolCall {
            name: tool_name, ..
        } => {
            agent.observable.tool_count += 1;
            agent.observable.last_tool = Some(tool_name.clone());
            agent.observable.status = AgentStatus::Running;
        }
        AgentEventPayload::ToolResult { .. } => {
            agent.observable.status = AgentStatus::Running;
        }
        AgentEventPayload::TokenUsage {
            input_tokens,
            output_tokens,
            ..
        } => {
            agent.observable.input_tokens = *input_tokens;
            agent.observable.output_tokens = *output_tokens;
        }
        AgentEventPayload::ModeChanged { mode } => {
            agent.observable.mode.clone_from(mode);
        }
        AgentEventPayload::AwaitingInput => {
            agent.observable.status = AgentStatus::WaitingForInput;
            agent.observable.turn_count += 1;
        }
        AgentEventPayload::Finished => {
            agent.observable.status = AgentStatus::Finished;
        }
        AgentEventPayload::Error { .. } => {
            agent.observable.status = AgentStatus::Error;
        }
        AgentEventPayload::Stream { .. } => {
            agent.observable.status = AgentStatus::Running;
        }
        AgentEventPayload::ThinkingStream { .. } => {
            agent.observable.status = AgentStatus::Running;
        }
        AgentEventPayload::ThinkingComplete { .. } => {}
        AgentEventPayload::MaxTurnsReached { .. } => {}
        AgentEventPayload::AutoContinuation { .. } => {}
        AgentEventPayload::MessageRouted { .. } => {}
        AgentEventPayload::ToolPermissionRequest { .. } => {}
        AgentEventPayload::UserQuestionRequest { .. } => {}
        AgentEventPayload::Rewound { .. } => {}
        AgentEventPayload::Compacted { .. } => {}
        AgentEventPayload::Interrupted => {
            agent.observable.status = AgentStatus::WaitingForInput;
        }
        AgentEventPayload::TurnDiffSummary { .. } => {}
    }
}
