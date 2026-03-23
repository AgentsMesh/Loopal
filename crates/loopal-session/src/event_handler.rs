//! AgentEvent → SessionState update logic. Routes events by `agent_name`:
//! root → main display, sub-agent → `agent_handler`. `MessageRouted` → global feed.

use loopal_protocol::{AgentEvent, AgentEventPayload, UserContent};

use crate::agent_handler::apply_agent_event;
use crate::inbox::try_forward_inbox;
use crate::message_log::record_message_routed;
use crate::state::SessionState;
use crate::thinking_display::format_thinking_summary;
use crate::tool_result_handler::handle_tool_result;
use crate::truncate::truncate_json;
use crate::types::{DisplayMessage, DisplayToolCall, PendingPermission};

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
        AgentEventPayload::ToolCall { id: _, name, input } => {
            flush_streaming(state);
            let tc = DisplayToolCall {
                name: name.clone(),
                status: "pending".to_string(),
                summary: if name == "AttemptCompletion" {
                    name.clone()
                } else {
                    format!("{}({})", name, truncate_json(&input, 60))
                },
                result: None,
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
            id: _,
            name,
            result,
            is_error,
        } => {
            handle_tool_result(state, name, result, is_error);
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
            state
                .messages
                .push(system_msg(&format!("Max turns reached ({turns})")));
        }
        AgentEventPayload::AutoContinuation {
            continuation,
            max_continuations,
        } => {
            state.messages.push(system_msg(&format!(
                "Output truncated (max_tokens). Auto-continuing ({continuation}/{max_continuations})"
            )));
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
        AgentEventPayload::Compacted { kept, removed } => {
            state.messages.push(system_msg(&format!(
                "Context compacted: removed {removed} messages, kept {kept}.",
            )));
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
    }
    None
}

/// Flush buffered streaming text into a DisplayMessage.
pub(crate) fn flush_streaming(state: &mut SessionState) {
    if !state.streaming_thinking.is_empty() {
        let thinking = std::mem::take(&mut state.streaming_thinking);
        let token_est = thinking.len() as u32 / 4;
        let summary = format_thinking_summary(&thinking, token_est);
        state.messages.push(DisplayMessage {
            role: "thinking".to_string(),
            content: summary,
            tool_calls: Vec::new(),
            image_count: 0,
        });
        state.thinking_active = false;
    }

    if !state.streaming_text.is_empty() {
        let text = std::mem::take(&mut state.streaming_text);
        if let Some(last) = state.messages.last_mut()
            && last.role == "assistant"
            && last.tool_calls.is_empty()
        {
            last.content.push_str(&text);
            return;
        }
        state.messages.push(DisplayMessage {
            role: "assistant".to_string(),
            content: text,
            tool_calls: Vec::new(),
            image_count: 0,
        });
    }
}

fn system_msg(content: &str) -> DisplayMessage {
    DisplayMessage {
        role: "system".into(),
        content: content.into(),
        tool_calls: Vec::new(),
        image_count: 0,
    }
}
