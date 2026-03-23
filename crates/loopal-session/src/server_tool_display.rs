//! Server-side tool event handling for TUI display.

use crate::helpers::flush_streaming;
use crate::state::SessionState;
use crate::truncate::truncate_json;
use crate::types::{DisplayMessage, DisplayToolCall};

/// Handle a ServerToolUse event — add a pending tool call entry with [server] label.
pub(crate) fn handle_server_tool_use(
    state: &mut SessionState,
    name: String,
    input: &serde_json::Value,
) {
    flush_streaming(state);
    let tc = DisplayToolCall {
        name: format!("{name} [server]"),
        status: "pending".to_string(),
        summary: format!("{}({})", name, truncate_json(input, 60)),
        result: None,
    };
    if let Some(last) = state.messages.last_mut()
        && last.role == "assistant"
    {
        last.tool_calls.push(tc);
        return;
    }
    state.messages.push(DisplayMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![tc],
        image_count: 0,
    });
}

/// Handle a ServerToolResult event — mark the last pending server tool as complete.
pub(crate) fn handle_server_tool_result(state: &mut SessionState) {
    let Some(msg) = state.messages.last_mut() else {
        return;
    };
    for tc in msg.tool_calls.iter_mut().rev() {
        if tc.status == "pending" && tc.name.contains("[server]") {
            tc.status = "success".to_string();
            tc.result = Some("Server-side search complete".to_string());
            return;
        }
    }
}
