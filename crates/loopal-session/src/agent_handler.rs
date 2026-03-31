//! Unified agent event handling — writes BOTH observable + conversation state.

use std::time::Instant;

use loopal_protocol::{AgentEventPayload, AgentStatus, UserContent};

use crate::agent_lifecycle::{extract_key_param, handle_idle};
use crate::conversation_display::{
    handle_auto_continuation, handle_compaction, handle_token_usage, push_system_msg,
};
use crate::state::SessionState;
use crate::thinking_display::handle_thinking_complete;
use crate::tool_result_handler::{
    ToolResultParams, handle_tool_batch_start, handle_tool_call, handle_tool_progress,
    handle_tool_result,
};
use crate::types::{PendingPermission, PendingQuestion, SessionMessage};

/// Handle an agent event — writes both observable metrics and conversation state.
pub(crate) fn apply_agent_event(
    state: &mut SessionState,
    name: &str,
    payload: AgentEventPayload,
) -> Option<UserContent> {
    let agent = state.agents.entry(name.to_string()).or_default();
    if agent.started_at.is_none() {
        agent.started_at = Some(Instant::now());
    }
    let obs = &mut agent.observable;
    let conv = &mut agent.conversation;

    match payload {
        AgentEventPayload::Stream { text } => {
            conv.begin_turn();
            conv.streaming_text.push_str(&text);
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::ThinkingStream { text } => {
            conv.begin_turn();
            conv.thinking_active = true;
            conv.streaming_thinking.push_str(&text);
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::ThinkingComplete { token_count } => {
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
            handle_tool_call(conv, id, tn, input);
        }
        AgentEventPayload::ToolResult {
            id,
            name: tn,
            result,
            is_error,
            duration_ms,
            is_completion,
            metadata,
        } => {
            handle_tool_result(
                conv,
                ToolResultParams {
                    id,
                    name: tn,
                    result,
                    is_error,
                    duration_ms,
                    is_completion,
                    metadata,
                },
            );
            obs.tools_in_flight = obs.tools_in_flight.saturating_sub(1);
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::ToolBatchStart { tool_ids } => {
            handle_tool_batch_start(conv, tool_ids);
        }
        AgentEventPayload::ToolProgress {
            id, output_tail, ..
        } => {
            handle_tool_progress(conv, id, output_tail);
        }
        AgentEventPayload::ToolPermissionRequest {
            id,
            name: tn,
            input,
        } => {
            conv.flush_streaming();
            conv.pending_permission = Some(PendingPermission {
                id,
                name: tn,
                input,
                relay_request_id: None,
            });
        }
        AgentEventPayload::UserQuestionRequest { id, questions } => {
            conv.flush_streaming();
            conv.pending_question = Some(PendingQuestion::new(id, questions));
        }
        AgentEventPayload::Error { message } => {
            conv.flush_streaming();
            conv.retry_banner = None;
            conv.messages.push(SessionMessage {
                role: "error".into(),
                content: message,
                tool_calls: Vec::new(),
                image_count: 0,
                skill_info: None,
            });
            obs.status = AgentStatus::Error;
        }
        AgentEventPayload::RetryError {
            message,
            attempt,
            max_attempts,
        } => {
            conv.retry_banner = Some(format!("{message} ({attempt}/{max_attempts})"));
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::RetryCleared => conv.retry_banner = None,
        AgentEventPayload::AwaitingInput => {
            return handle_idle(state, name, AgentStatus::WaitingForInput);
        }
        AgentEventPayload::Finished => {
            return handle_idle(state, name, AgentStatus::Finished);
        }
        AgentEventPayload::Interrupted => {
            return handle_idle(state, name, AgentStatus::WaitingForInput);
        }
        AgentEventPayload::MaxTurnsReached { turns } => {
            conv.flush_streaming();
            push_system_msg(conv, &format!("Max turns reached ({turns})"));
        }
        AgentEventPayload::AutoContinuation {
            continuation,
            max_continuations,
        } => {
            handle_auto_continuation(conv, continuation, max_continuations);
        }
        AgentEventPayload::TokenUsage {
            input_tokens,
            output_tokens,
            context_window,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            ..
        } => {
            handle_token_usage(
                conv,
                input_tokens,
                output_tokens,
                context_window,
                cache_creation_input_tokens,
                cache_read_input_tokens,
            );
            obs.input_tokens = input_tokens;
            obs.output_tokens = output_tokens;
        }
        AgentEventPayload::ModeChanged { mode } => obs.mode.clone_from(&mode),
        AgentEventPayload::Rewound { remaining_turns } => {
            crate::rewind::truncate_display_to_turn(conv, remaining_turns);
        }
        AgentEventPayload::Compacted {
            kept,
            removed,
            tokens_before,
            tokens_after,
            strategy,
        } => {
            handle_compaction(conv, kept, removed, tokens_before, tokens_after, &strategy);
        }
        AgentEventPayload::Started => obs.status = AgentStatus::Running,
        AgentEventPayload::ServerToolUse {
            id,
            name: tn,
            input,
        } => {
            crate::server_tool_display::handle_server_tool_use(conv, id, tn, &input);
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::ServerToolResult {
            tool_use_id,
            content,
        } => {
            crate::server_tool_display::handle_server_tool_result(conv, &tool_use_id, &content);
            obs.status = AgentStatus::Running;
        }
        AgentEventPayload::SubAgentSpawned { .. }
        | AgentEventPayload::MessageRouted { .. }
        | AgentEventPayload::TurnDiffSummary { .. } => {}
        AgentEventPayload::AutoModeDecision {
            tool_name,
            decision,
            reason,
            duration_ms,
        } => {
            let label = if decision == "allow" {
                "auto-allowed"
            } else {
                "auto-denied"
            };
            let t = if duration_ms > 0 {
                format!("({duration_ms}ms)")
            } else {
                "(cached)".into()
            };
            push_system_msg(conv, &format!("[{label}] {tool_name}: {reason} {t}"));
        }
    }
    crate::agent_lifecycle::auto_return_on_error(state, name);
    None
}
