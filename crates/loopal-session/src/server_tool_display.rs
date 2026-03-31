//! Server-side tool event handling for TUI display.

use serde_json::Value;

use crate::agent_conversation::AgentConversation;
use crate::truncate::truncate_json;
use crate::types::{SessionMessage, SessionToolCall, ToolCallStatus};

/// Handle a ServerToolUse event — add a pending tool call entry.
pub(crate) fn handle_server_tool_use(
    conv: &mut AgentConversation,
    id: String,
    name: String,
    input: &Value,
) {
    conv.flush_streaming();
    let tc = SessionToolCall {
        id,
        name: name.clone(),
        status: ToolCallStatus::Pending,
        summary: format!("{}({})", name, truncate_json(input, 60)),
        result: None,
        tool_input: Some(input.clone()),
        batch_id: None,
        started_at: None,
        duration_ms: None,
        progress_tail: None,
        metadata: None,
    };
    if let Some(last) = conv.messages.last_mut()
        && last.role == "assistant"
    {
        last.tool_calls.push(tc);
        return;
    }
    conv.messages.push(SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![tc],
        image_count: 0,
        skill_info: None,
    });
}

/// Handle a ServerToolResult event — fill in the actual result content.
pub(crate) fn handle_server_tool_result(
    conv: &mut AgentConversation,
    tool_use_id: &str,
    content: &Value,
) {
    let Some(msg) = conv.messages.last_mut() else {
        return;
    };
    if let Some(tc) = msg.tool_calls.iter_mut().rfind(|tc| tc.id == tool_use_id) {
        tc.status = ToolCallStatus::Success;
        tc.result = Some(format_server_tool_content(content));
    }
}

/// Extract human-readable text from server tool result JSON.
/// Also used by projection.rs to format stored ServerToolResult content.
pub fn format_server_tool_content(content: &Value) -> String {
    let raw = extract_content_text(content);
    crate::truncate::truncate_result_for_storage(&raw)
}

fn extract_content_text(content: &Value) -> String {
    if let Some(arr) = content.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match item_type {
                "web_search_result" => {
                    if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                        let url = item.get("url").and_then(|v| v.as_str()).unwrap_or("");
                        parts.push(format!("{title}\n  {url}"));
                    }
                }
                "code_execution_output" => {
                    if let Some(output) = item.get("output").and_then(|v| v.as_str()) {
                        parts.push(output.trim_end().to_string());
                    }
                }
                _ => {
                    // Generic: try common text fields, then title+url (Google format)
                    if let Some(text) = item
                        .get("output")
                        .or_else(|| item.get("text"))
                        .or_else(|| item.get("content"))
                        .and_then(|v| v.as_str())
                    {
                        parts.push(text.trim_end().to_string());
                    } else if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
                        let url = item.get("url").and_then(|v| v.as_str()).unwrap_or("");
                        parts.push(format!("{title}\n  {url}"));
                    }
                }
            }
        }
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    // Fallback: compact JSON (truncated)
    truncate_json(content, 200)
}
