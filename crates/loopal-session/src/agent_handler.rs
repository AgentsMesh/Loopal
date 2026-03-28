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
            name: tool_name,
            input,
            ..
        } => {
            agent.observable.tool_count += 1;
            agent.observable.tools_in_flight += 1;
            agent.observable.last_tool = Some(extract_key_param(tool_name, input));
            agent.observable.status = AgentStatus::Running;
        }
        AgentEventPayload::ToolResult { .. } => {
            agent.observable.tools_in_flight = agent.observable.tools_in_flight.saturating_sub(1);
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
        AgentEventPayload::ToolProgress { .. } => {}
        AgentEventPayload::ToolBatchStart { .. } => {}
        AgentEventPayload::Interrupted => {
            agent.observable.status = AgentStatus::WaitingForInput;
        }
        AgentEventPayload::TurnDiffSummary { .. } => {}
        AgentEventPayload::ServerToolUse { .. } | AgentEventPayload::ServerToolResult { .. } => {
            agent.observable.status = AgentStatus::Running;
        }
        AgentEventPayload::RetryError { .. } => {
            agent.observable.status = AgentStatus::Running;
        }
        AgentEventPayload::RetryCleared => {}
        AgentEventPayload::SubAgentSpawned { .. } => {}
    }
}

/// Register a newly spawned agent with parent/child topology.
pub(crate) fn register_spawned_agent(
    state: &mut SessionState,
    name: &str,
    parent: Option<&str>,
    model: Option<&str>,
) {
    let agent = state.agents.entry(name.to_string()).or_default();
    agent.parent = parent.map(String::from);
    if let Some(m) = model {
        agent.observable.model = m.to_string();
    }
    // Register as child of parent
    if let Some(p) = parent {
        let child_name = name.to_string();
        if let Some(parent_agent) = state.agents.get_mut(p) {
            if !parent_agent.children.contains(&child_name) {
                parent_agent.children.push(child_name);
            }
        }
    }
}

/// Extract the most informative parameter from a tool call for display.
fn extract_key_param(tool_name: &str, input: &serde_json::Value) -> String {
    let key = match tool_name {
        "Read" | "Write" | "Edit" | "MultiEdit" => "file_path",
        "Bash" => "command",
        "Grep" => "pattern",
        "Glob" => "pattern",
        _ => return tool_name.to_string(),
    };
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| {
            if s.len() > 40 {
                let truncated: String = s.chars().take(37).collect();
                format!("{tool_name}({truncated}...)")
            } else {
                format!("{tool_name}({s})")
            }
        })
        .unwrap_or_else(|| tool_name.to_string())
}
