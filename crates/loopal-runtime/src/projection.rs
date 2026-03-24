//! Projection: convert `Vec<Message>` → `Vec<DisplayMessage>`.
//!
//! Used when restoring a session so the TUI can display historical messages
//! without replaying the full agent event stream.
//!
//! Must mirror the display semantics of `event_handler.rs` — in particular,
//! AttemptCompletion tool results are promoted to assistant messages rather
//! than shown as tool output.

use std::collections::HashMap;

use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_session::types::{DisplayMessage, DisplayToolCall};
use loopal_tool_api::COMPLETION_PREFIX;

/// Project a slice of Messages into DisplayMessages suitable for TUI rendering.
pub fn project_messages(messages: &[Message]) -> Vec<DisplayMessage> {
    let mut display: Vec<DisplayMessage> = Vec::new();
    // Maps tool_use_id → (display_index, tool_call_index, tool_name)
    let mut tool_index: HashMap<String, (usize, usize, String)> = HashMap::new();

    for msg in messages {
        let role = role_str(&msg.role);
        let mut content_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<DisplayToolCall> = Vec::new();
        let mut image_count: usize = 0;

        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => content_parts.push(text.clone()),
                ContentBlock::ToolUse { id, name, input } => {
                    let tc_idx = tool_calls.len();
                    tool_calls.push(DisplayToolCall {
                        name: name.clone(),
                        status: "pending".to_string(),
                        summary: format!("{}({})", name, summarize_input(input)),
                        result: None,
                    });
                    tool_index.insert(id.clone(), (display.len(), tc_idx, name.clone()));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    back_patch(&mut display, &tool_index, tool_use_id, content, *is_error);
                }
                ContentBlock::Image { .. } => {
                    content_parts.push("[image]".to_string());
                    image_count += 1;
                }
                ContentBlock::Thinking { .. } => {}
                ContentBlock::ServerToolUse { name, input, .. } => {
                    let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
                    tool_calls.push(DisplayToolCall {
                        name: format!("{name} [server]"),
                        status: "success".to_string(),
                        summary: format!("{name}({query})"),
                        result: None,
                    });
                }
                ContentBlock::ServerToolResult { .. } => {}
            }
        }

        let content = content_parts.join("");
        if content.is_empty() && tool_calls.is_empty() {
            continue;
        }
        display.push(DisplayMessage {
            role,
            content,
            tool_calls,
            image_count,
        });
    }

    display
}

/// Back-patch a ToolResult into the matching ToolUse's DisplayToolCall.
/// AttemptCompletion results are promoted to a standalone assistant message
/// (matching event_handler.rs behavior).
fn back_patch(
    display: &mut Vec<DisplayMessage>,
    index: &HashMap<String, (usize, usize, String)>,
    tool_use_id: &str,
    result: &str,
    is_error: bool,
) {
    let Some(&(di, ti, ref name)) = index.get(tool_use_id) else {
        return;
    };
    let is_completion = name == "AttemptCompletion" && !is_error;
    if let Some(msg) = display.get_mut(di)
        && let Some(tc) = msg.tool_calls.get_mut(ti)
    {
        tc.status = if is_error { "error" } else { "success" }.to_string();
        if !is_completion {
            tc.result = Some(truncate_for_display(result));
        }
    }
    // Promote AttemptCompletion to an assistant message
    if is_completion {
        let content = result.strip_prefix(COMPLETION_PREFIX).unwrap_or(result);
        display.push(DisplayMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
            tool_calls: Vec::new(),
            image_count: 0,
        });
    }
}

fn summarize_input(input: &serde_json::Value) -> String {
    let s = input.to_string();
    if s.len() <= 60 {
        s
    } else {
        format!("{}...", &s[..57])
    }
}

fn truncate_for_display(s: &str) -> String {
    const MAX_LINES: usize = 200;
    const MAX_BYTES: usize = 10_000;
    let line_limited: String = s.lines().take(MAX_LINES).collect::<Vec<_>>().join("\n");
    if line_limited.len() <= MAX_BYTES {
        line_limited
    } else {
        line_limited[..MAX_BYTES].to_string()
    }
}

fn role_str(role: &MessageRole) -> String {
    match role {
        MessageRole::User => "user".to_string(),
        MessageRole::Assistant => "assistant".to_string(),
        MessageRole::System => "system".to_string(),
    }
}
