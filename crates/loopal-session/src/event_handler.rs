//! AgentEvent → SessionState update logic. Routes events by `agent_name`:
//! root → main display, sub-agent → `agent_handler`. `MessageRouted` → global feed.

use std::time::Instant;

use loopal_protocol::{AgentEvent, AgentEventPayload, UserContent};

use crate::agent_handler::apply_agent_event;
use crate::helpers::{flush_streaming, push_system_msg};
use crate::inbox::try_forward_inbox;
use crate::message_log::record_message_routed;
use crate::state::SessionState;
use crate::thinking_display::format_thinking_summary;
use crate::tool_result_handler::{
    handle_tool_batch_start, handle_tool_progress, handle_tool_result,
};
use crate::truncate::truncate_json;
use crate::types::{DisplayMessage, DisplayToolCall, PendingPermission, ToolCallStatus};

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

/// Handle a root-agent event (main display updates).
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
            state.thinking_active = false;
            state.thinking_tokens += token_count;
            if !state.streaming_thinking.is_empty() {
                let thinking = std::mem::take(&mut state.streaming_thinking);
                let summary = format_thinking_summary(&thinking, token_count);
                state.messages.push(DisplayMessage {
                    role: "thinking".to_string(),
                    content: summary,
                    tool_calls: Vec::new(),
                    image_count: 0,
                });
            }
        }
        AgentEventPayload::ToolCall { id, name, input } => {
            flush_streaming(state);
            let tc = DisplayToolCall {
                id: id.clone(),
                name: name.clone(),
                status: ToolCallStatus::Pending,
                summary: if name == "AttemptCompletion" {
                    name.clone()
                } else {
                    format!("{}({})", name, truncate_json(&input, 60))
                },
                result: None,
                tool_input: Some(input),
                batch_id: None,
                started_at: Some(Instant::now()),
                duration_ms: None,
                progress_tail: None,
            };
            if let Some(last) = state.messages.last_mut()
                && last.role == "assistant"
            {
                last.tool_calls.push(tc);
                return None;
            }
            state.messages.push(DisplayMessage {
                role: "assistant".to_string(),
                content: String::new(),
                tool_calls: vec![tc],
                image_count: 0,
            });
        }
        AgentEventPayload::ToolResult {
            id,
            name,
            result,
            is_error,
            duration_ms,
        } => {
            handle_tool_result(state, id, name, result, is_error, duration_ms);
        }
        AgentEventPayload::ToolPermissionRequest { id, name, input } => {
            flush_streaming(state);
            state.pending_permission = Some(PendingPermission { id, name, input });
        }
        AgentEventPayload::Error { message } => {
            flush_streaming(state);
            state.messages.push(DisplayMessage {
                role: "error".into(),
                content: message,
                tool_calls: Vec::new(),
                image_count: 0,
            });
        }
        AgentEventPayload::AwaitingInput => {
            flush_streaming(state);
            state.end_turn();
            state.turn_count += 1;
            state.agent_idle = true;
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
            push_system_msg(
                state,
                &format!(
                    "Output truncated (max_tokens). Auto-continuing ({continuation}/{max_continuations})"
                ),
            );
        }
        AgentEventPayload::TokenUsage {
            input_tokens,
            output_tokens,
            context_window,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            thinking_tokens: _,
        } => {
            state.input_tokens = input_tokens;
            state.output_tokens = output_tokens;
            state.context_window = context_window;
            state.cache_creation_tokens = cache_creation_input_tokens;
            state.cache_read_tokens = cache_read_input_tokens;
            if input_tokens == 0 && output_tokens == 0 {
                state.thinking_tokens = 0;
            }
        }
        AgentEventPayload::ModeChanged { mode } => {
            state.mode = mode;
        }
        AgentEventPayload::Started => {}
        AgentEventPayload::Finished => {
            flush_streaming(state);
            state.end_turn();
            state.agent_idle = true;
        }
        AgentEventPayload::MessageRouted { .. } => {}
        AgentEventPayload::UserQuestionRequest { id, questions } => {
            flush_streaming(state);
            state.pending_question = Some(super::types::PendingQuestion::new(id, questions));
        }
        AgentEventPayload::Rewound { remaining_turns } => {
            crate::rewind::truncate_display_to_turn(state, remaining_turns);
        }
        AgentEventPayload::Compacted {
            kept,
            removed,
            tokens_before,
            tokens_after,
            strategy,
        } => {
            let freed = tokens_before.saturating_sub(tokens_after);
            let pct = if tokens_before > 0 {
                freed * 100 / tokens_before
            } else {
                0
            };
            push_system_msg(
                state,
                &format!(
                    "Context compacted ({strategy}): {removed} messages removed, \
                     {kept} kept. {tokens_before}→{tokens_after} tokens ({pct}% freed).",
                ),
            );
        }
        AgentEventPayload::ToolBatchStart { tool_ids } => {
            handle_tool_batch_start(state, tool_ids);
        }
        AgentEventPayload::ToolProgress {
            id, output_tail, ..
        } => {
            handle_tool_progress(state, id, output_tail);
        }
        AgentEventPayload::Interrupted => {
            flush_streaming(state);
            state.end_turn();
            state.agent_idle = true;
            return try_forward_inbox(state);
        }
        AgentEventPayload::TurnDiffSummary { .. } => {
            // Informational — TUI can display file diff summary in status bar.
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
    }
    None
}
