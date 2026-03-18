//! AgentEvent → SessionState update logic. Routes events by `agent_name`:
//! root → main display, sub-agent → `agent_handler`. `MessageRouted` → global feed.

use loopagent_types::event::{AgentEvent, AgentEventPayload};

use crate::agent_handler::apply_agent_event;
use crate::truncate::{truncate_json, truncate_result_for_storage};
use crate::message_log::MessageLogEntry;
use crate::state::SessionState;
use crate::types::{DisplayMessage, DisplayToolCall, PendingPermission};

/// Handle an AgentEvent by mutating SessionState in-place.
/// Returns `Some(text)` if an inbox message should be forwarded (agent became idle).
pub fn apply_event(state: &mut SessionState, event: AgentEvent) -> Option<String> {
    // Handle MessageRouted globally regardless of agent_name
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
fn apply_root_event(state: &mut SessionState, payload: AgentEventPayload) -> Option<String> {
    match payload {
        AgentEventPayload::Stream { text } => {
            state.begin_turn();
            state.streaming_text.push_str(&text);
        }
        AgentEventPayload::ToolCall { id: _, name, input } => {
            flush_streaming(state);
            let tc = DisplayToolCall {
                name: name.clone(),
                status: "pending".to_string(),
                summary: if name == "AttemptCompletion" { name.clone() }
                    else { format!("{}({})", name, truncate_json(&input, 60)) },
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
            });
        }
        AgentEventPayload::ToolResult { id: _, name, result, is_error } => {
            handle_tool_result(state, name, result, is_error);
        }
        AgentEventPayload::ToolPermissionRequest { id, name, input } => {
            flush_streaming(state);
            state.pending_permission = Some(PendingPermission { id, name, input });
        }
        AgentEventPayload::Error { message } => {
            flush_streaming(state);
            state.messages.push(DisplayMessage {
                role: "error".to_string(),
                content: message,
                tool_calls: Vec::new(),
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
            state.messages.push(DisplayMessage {
                role: "system".to_string(),
                content: format!("Max turns reached ({})", turns),
                tool_calls: Vec::new(),
            });
        }
        AgentEventPayload::AutoContinuation { continuation, max_continuations } => {
            state.messages.push(DisplayMessage {
                role: "system".to_string(),
                content: format!(
                    "Output truncated (max_tokens). Auto-continuing ({}/{})",
                    continuation, max_continuations
                ),
                tool_calls: Vec::new(),
            });
        }
        AgentEventPayload::TokenUsage {
            input_tokens, output_tokens, context_window,
            cache_creation_input_tokens, cache_read_input_tokens,
        } => {
            state.input_tokens = input_tokens;
            state.output_tokens = output_tokens;
            state.context_window = context_window;
            state.cache_creation_tokens = cache_creation_input_tokens;
            state.cache_read_tokens = cache_read_input_tokens;
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
        AgentEventPayload::MessageRouted { .. } => {
            // Handled globally in apply_event() before this match.
        }
    }
    None
}

/// Handle ToolResult: update status, and promote AttemptCompletion to assistant message.
fn handle_tool_result(state: &mut SessionState, name: String, result: String, is_error: bool) {
    let status = if is_error { "error" } else { "success" };
    let is_completion = name == "AttemptCompletion" && !is_error;
    'outer: for msg in state.messages.iter_mut().rev() {
        for tc in msg.tool_calls.iter_mut().rev() {
            if tc.name == name && tc.status == "pending" {
                tc.status = status.to_string();
                if !is_completion { tc.result = Some(truncate_result_for_storage(&result)); }
                break 'outer;
            }
        }
    }
    // Promote AttemptCompletion to assistant message (prefix from AttemptCompletionTool)
    if is_completion {
        let content = result.strip_prefix("Completion acknowledged: ").unwrap_or(&result);
        state.messages.push(DisplayMessage {
            role: "assistant".into(),
            content: content.to_string(),
            tool_calls: Vec::new(),
        });
    }
}

/// Flush buffered streaming text into a DisplayMessage.
pub(crate) fn flush_streaming(state: &mut SessionState) {
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
        });
    }
}

/// Try forwarding a queued inbox message when agent is idle.
pub(crate) fn try_forward_inbox(state: &mut SessionState) -> Option<String> {
    if state.agent_idle
        && let Some(text) = state.inbox.pop_front()
    {
        state.agent_idle = false;
        state.begin_turn();
        state.messages.push(DisplayMessage {
            role: "user".to_string(),
            content: text.clone(),
            tool_calls: Vec::new(),
        });
        return Some(text);
    }
    None
}

/// Record a MessageRouted event to the global feed and per-agent logs.
fn record_message_routed(state: &mut SessionState, source: &str, target: &str, preview: &str) {
    let entry = MessageLogEntry::new(source, target, preview);
    state.message_feed.record(entry.clone());
    // Record to source agent's log
    if let Some(agent) = state.agents.get_mut(source) {
        agent.message_log.push(entry.clone());
    }
    // Record to target agent's log
    if let Some(agent) = state.agents.get_mut(target) {
        agent.message_log.push(entry);
    }
}
