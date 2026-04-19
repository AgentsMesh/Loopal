//! Unified agent event handling — writes BOTH observable + conversation state.

use std::time::Instant;

use loopal_protocol::{AgentEventPayload, AgentStatus};

use crate::agent_event_helpers::{
    apply_auto_continuation, apply_auto_mode_decision, apply_compaction_event, apply_error_event,
    apply_token_usage, apply_tool_permission_request, apply_tool_result_event,
    apply_user_question_request, clear_all_panel_caches_for_resume,
};
use crate::agent_lifecycle::{extract_key_param, handle_idle, post_event_cleanup};
use crate::state::SessionState;
use crate::thinking_display::handle_thinking_complete;
use crate::tool_result_handler::{handle_tool_batch_start, handle_tool_call, handle_tool_progress};

/// Handle an agent event — writes both observable metrics and conversation state.
pub(crate) fn apply_agent_event(state: &mut SessionState, name: &str, payload: AgentEventPayload) {
    let agent = state.agents.entry(name.to_string()).or_default();
    if agent.started_at.is_none() {
        agent.started_at = Some(Instant::now());
    }
    let obs = &mut agent.observable;
    let conv = &mut agent.conversation;
    let mut sync_parent = false;

    match payload {
        AgentEventPayload::Stream { text } => {
            conv.begin_turn();
            conv.mark_active();
            conv.streaming_text.push_str(&text);
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::ThinkingStream { text } => {
            conv.begin_turn();
            conv.mark_active();
            conv.thinking_active = true;
            conv.streaming_thinking.push_str(&text);
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::ThinkingComplete { token_count } => {
            conv.mark_active();
            handle_thinking_complete(conv, token_count);
        }
        AgentEventPayload::ToolCall {
            id,
            name: tn,
            input,
        } => {
            obs.tool_count += 1;
            obs.tools_in_flight += 1;
            obs.last_tool = Some(extract_key_param(&tn, &input));
            obs.status = AgentStatus::Running;
            conv.mark_active();
            handle_tool_call(conv, id, tn, input);
            sync_parent = true;
        }
        payload @ AgentEventPayload::ToolResult { .. } => {
            conv.mark_active();
            apply_tool_result_event(conv, obs, payload);
            sync_parent = true;
        }
        AgentEventPayload::ToolBatchStart { tool_ids } => {
            conv.mark_active();
            handle_tool_batch_start(conv, tool_ids);
        }
        AgentEventPayload::ToolProgress {
            id, output_tail, ..
        } => {
            conv.mark_active();
            handle_tool_progress(conv, id, output_tail);
            sync_parent = true;
        }
        payload @ AgentEventPayload::ToolPermissionRequest { .. } => {
            apply_tool_permission_request(conv, payload);
        }
        payload @ AgentEventPayload::UserQuestionRequest { .. } => {
            apply_user_question_request(conv, payload);
        }
        AgentEventPayload::Error { message } => apply_error_event(conv, obs, message),
        AgentEventPayload::RetryError {
            message,
            attempt,
            max_attempts,
        } => {
            conv.retry_banner = Some(format!("{message} ({attempt}/{max_attempts})"));
            obs.status = AgentStatus::Running;
            conv.mark_active();
        }
        AgentEventPayload::RetryCleared => conv.retry_banner = None,
        AgentEventPayload::AwaitingInput => {
            handle_idle(state, name, AgentStatus::WaitingForInput);
            return;
        }
        AgentEventPayload::Finished => {
            handle_idle(state, name, AgentStatus::Finished);
            return;
        }
        AgentEventPayload::Interrupted => {
            handle_idle(state, name, AgentStatus::WaitingForInput);
            return;
        }
        payload @ AgentEventPayload::AutoContinuation { .. } => {
            apply_auto_continuation(conv, payload);
        }
        payload @ AgentEventPayload::TokenUsage { .. } => apply_token_usage(conv, obs, payload),
        AgentEventPayload::ModeChanged { mode } => obs.mode.clone_from(&mode),
        AgentEventPayload::Rewound { remaining_turns } => {
            crate::rewind::truncate_display_to_turn(conv, remaining_turns);
        }
        payload @ AgentEventPayload::Compacted { .. } => apply_compaction_event(conv, payload),
        AgentEventPayload::Started => {
            obs.status = AgentStatus::Running;
            conv.mark_active();
        }
        AgentEventPayload::Running => {
            conv.begin_turn();
            conv.mark_active();
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::ServerToolUse {
            id,
            name: tn,
            input,
        } => {
            conv.mark_active();
            crate::server_tool_display::handle_server_tool_use(conv, id, tn, &input);
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::ServerToolResult {
            tool_use_id,
            content,
        } => {
            conv.mark_active();
            crate::server_tool_display::handle_server_tool_result(conv, &tool_use_id, &content);
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::SubAgentSpawned { .. }
        | AgentEventPayload::MessageRouted { .. }
        | AgentEventPayload::TurnDiffSummary { .. }
        | AgentEventPayload::TurnCompleted { .. }
        | AgentEventPayload::McpStatusReport { .. } => {}
        payload @ AgentEventPayload::SessionResumed { .. } => {
            clear_all_panel_caches_for_resume(state, payload);
            return;
        }
        AgentEventPayload::BgTaskSpawned { .. }
        | AgentEventPayload::BgTaskOutput { .. }
        | AgentEventPayload::BgTaskCompleted { .. } => {
            crate::bg_task_state::apply(state, payload);
            return;
        }
        AgentEventPayload::TasksChanged { .. } => {
            crate::task_state::apply(state, payload);
            return;
        }
        AgentEventPayload::CronsChanged { .. } => {
            crate::cron_state::apply(state, payload);
            return;
        }
        payload @ AgentEventPayload::AutoModeDecision { .. } => {
            apply_auto_mode_decision(conv, payload);
        }
    }
    post_event_cleanup(state, name, sync_parent);
}
