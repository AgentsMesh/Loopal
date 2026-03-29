//! AgentEvent → SessionState update logic. Routes by `agent_name`.

use loopal_protocol::{AgentEvent, AgentEventPayload, UserContent};

use crate::agent_handler::apply_agent_event;
use crate::helpers::{
    flush_streaming, handle_auto_continuation, handle_compaction, handle_token_usage,
    push_system_msg,
};
use crate::inbox::try_forward_inbox;
use crate::message_log::record_message_routed;
use crate::state::SessionState;
use crate::thinking_display::handle_thinking_complete;
use crate::tool_result_handler::{
    ToolResultParams, handle_tool_batch_start, handle_tool_call, handle_tool_progress,
    handle_tool_result,
};
use crate::types::{DisplayMessage, PendingPermission};

/// Handle an AgentEvent. Returns `Some(content)` if an inbox message should be forwarded.
pub fn apply_event(state: &mut SessionState, event: AgentEvent) -> Option<UserContent> {
    if let AgentEventPayload::MessageRouted {
        ref source,
        ref target,
        ref content_preview,
    } = event.payload
    {
        record_message_routed(state, source, target, content_preview);
    }

    match event.agent_name {
        None => apply_root_event(state, event.payload),
        Some(name) => {
            apply_agent_event(state, &name, event.payload);
            None
        }
    }
}

fn apply_root_event(state: &mut SessionState, payload: AgentEventPayload) -> Option<UserContent> {
    match payload {
        AgentEventPayload::Stream { text } => {
            state.begin_turn();
            state.streaming_text.push_str(&text);
        }
        AgentEventPayload::ThinkingStream { text } => {
            state.begin_turn();
            state.thinking_active = true;
            state.streaming_thinking.push_str(&text);
        }
        AgentEventPayload::ThinkingComplete { token_count } => {
            handle_thinking_complete(state, token_count);
        }
        AgentEventPayload::ToolCall { id, name, input } => {
            handle_tool_call(state, id, name, input);
        }
        AgentEventPayload::ToolResult {
            id,
            name,
            result,
            is_error,
            duration_ms,
            is_completion,
            metadata,
        } => {
            handle_tool_result(
                state,
                ToolResultParams {
                    id,
                    name,
                    result,
                    is_error,
                    duration_ms,
                    is_completion,
                    metadata,
                },
            );
        }
        AgentEventPayload::ToolPermissionRequest { id, name, input } => {
            flush_streaming(state);
            state.pending_permission = Some(PendingPermission {
                id,
                name,
                input,
                relay_request_id: None,
            });
        }
        AgentEventPayload::Error { message } => {
            flush_streaming(state);
            state.retry_banner = None;
            state.messages.push(DisplayMessage {
                role: "error".into(),
                content: message,
                tool_calls: Vec::new(),
                image_count: 0,
            });
        }
        AgentEventPayload::RetryError {
            message,
            attempt,
            max_attempts,
        } => {
            state.retry_banner = Some(format!("{message} ({attempt}/{max_attempts})"));
        }
        AgentEventPayload::RetryCleared => state.retry_banner = None,
        AgentEventPayload::AwaitingInput => {
            tracing::debug!("TUI: agent idle (AwaitingInput)");
            flush_streaming(state);
            state.end_turn();
            state.turn_count += 1;
            state.agent_idle = true;
            state.retry_banner = None;
            return try_forward_inbox(state);
        }
        AgentEventPayload::MaxTurnsReached { turns } => {
            flush_streaming(state);
            push_system_msg(state, &format!("Max turns reached ({turns})"));
        }
        AgentEventPayload::AutoContinuation {
            continuation,
            max_continuations,
        } => {
            handle_auto_continuation(state, continuation, max_continuations);
        }
        AgentEventPayload::TokenUsage {
            input_tokens,
            output_tokens,
            context_window,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            thinking_tokens: _,
        } => {
            handle_token_usage(
                state,
                input_tokens,
                output_tokens,
                context_window,
                cache_creation_input_tokens,
                cache_read_input_tokens,
            );
        }
        AgentEventPayload::ModeChanged { mode } => state.mode = mode,
        AgentEventPayload::MessageRouted { .. }
        | AgentEventPayload::Started
        | AgentEventPayload::TurnDiffSummary { .. } => {}
        AgentEventPayload::Finished => {
            flush_streaming(state);
            state.end_turn();
            state.agent_idle = true;
            state.retry_banner = None;
        }
        AgentEventPayload::UserQuestionRequest { id, questions } => {
            flush_streaming(state);
            state.pending_question = Some(super::types::PendingQuestion::new(id, questions));
        }
        AgentEventPayload::Rewound { remaining_turns } => {
            crate::rewind::truncate_display_to_turn(state, remaining_turns)
        }
        AgentEventPayload::Compacted {
            kept,
            removed,
            tokens_before,
            tokens_after,
            strategy,
        } => {
            handle_compaction(state, kept, removed, tokens_before, tokens_after, &strategy);
        }
        AgentEventPayload::ToolBatchStart { tool_ids } => handle_tool_batch_start(state, tool_ids),
        AgentEventPayload::ToolProgress {
            id, output_tail, ..
        } => handle_tool_progress(state, id, output_tail),
        AgentEventPayload::Interrupted => {
            tracing::debug!("TUI: agent interrupted");
            flush_streaming(state);
            state.end_turn();
            state.agent_idle = true;
            state.retry_banner = None;
            return try_forward_inbox(state);
        }
        AgentEventPayload::ServerToolUse { id, name, input } => {
            crate::server_tool_display::handle_server_tool_use(state, id, name, &input);
        }
        AgentEventPayload::ServerToolResult {
            tool_use_id,
            content,
        } => {
            crate::server_tool_display::handle_server_tool_result(state, &tool_use_id, &content);
        }
        AgentEventPayload::SubAgentSpawned {
            name,
            parent,
            model,
            ..
        } => {
            crate::agent_handler::register_spawned_agent(
                state,
                &name,
                parent.as_deref(),
                model.as_deref(),
            );
        }
    }
    None
}
