//! AgentEvent → session-level state.
//!
//! Per-agent state (conversation, observable, tasks, crons, bg_tasks,
//! mode, model) is owned by the per-agent `ViewClient` — `SessionState`
//! only tracks active_view, root_session_id, and mcp_status. Every
//! variant of `AgentEventPayload` is listed explicitly so a future
//! variant addition triggers a compile error here, forcing a
//! deliberate decision.

use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::state::{ROOT_AGENT, SessionState};

pub fn apply_event(state: &mut SessionState, event: AgentEvent) {
    match &event.payload {
        AgentEventPayload::SessionResumed { session_id, .. } => {
            state.root_session_id = Some(session_id.clone());
        }
        AgentEventPayload::McpStatusReport { servers } => {
            state.mcp_status = Some(servers.clone());
        }
        AgentEventPayload::Finished | AgentEventPayload::Error { .. } => {
            // Auto-return to root when the viewed agent terminates.
            let agent_name = event
                .agent_name
                .as_ref()
                .map(|a| a.to_string())
                .unwrap_or_else(|| ROOT_AGENT.to_string());
            if agent_name == state.active_view && agent_name != ROOT_AGENT {
                state.active_view = ROOT_AGENT.to_string();
            }
        }
        // Per-agent events handled by ViewClient reducer.
        AgentEventPayload::SubAgentSpawned { .. }
        | AgentEventPayload::Started
        | AgentEventPayload::Running
        | AgentEventPayload::AwaitingInput
        | AgentEventPayload::Interrupted
        | AgentEventPayload::Stream { .. }
        | AgentEventPayload::ThinkingStream { .. }
        | AgentEventPayload::ThinkingComplete { .. }
        | AgentEventPayload::ToolCall { .. }
        | AgentEventPayload::ToolResult { .. }
        | AgentEventPayload::ToolBatchStart { .. }
        | AgentEventPayload::ToolProgress { .. }
        | AgentEventPayload::ToolPermissionRequest { .. }
        | AgentEventPayload::ToolPermissionResolved { .. }
        | AgentEventPayload::UserQuestionRequest { .. }
        | AgentEventPayload::UserQuestionResolved { .. }
        | AgentEventPayload::UserMessageQueued { .. }
        | AgentEventPayload::TokenUsage { .. }
        | AgentEventPayload::TurnCompleted { .. }
        | AgentEventPayload::TurnDiffSummary { .. }
        | AgentEventPayload::ModeChanged { .. }
        | AgentEventPayload::RetryError { .. }
        | AgentEventPayload::RetryCleared
        | AgentEventPayload::AutoContinuation { .. }
        | AgentEventPayload::AutoModeDecision { .. }
        | AgentEventPayload::Compacted { .. }
        | AgentEventPayload::Rewound { .. }
        | AgentEventPayload::ServerToolUse { .. }
        | AgentEventPayload::ServerToolResult { .. }
        | AgentEventPayload::InboxEnqueued { .. }
        | AgentEventPayload::InboxConsumed { .. }
        | AgentEventPayload::MessageRouted { .. }
        | AgentEventPayload::SessionResumeWarnings { .. }
        | AgentEventPayload::BgTaskSpawned { .. }
        | AgentEventPayload::BgTaskOutput { .. }
        | AgentEventPayload::BgTaskCompleted { .. }
        | AgentEventPayload::TasksChanged { .. }
        | AgentEventPayload::CronsChanged { .. } => {}
    }
}
