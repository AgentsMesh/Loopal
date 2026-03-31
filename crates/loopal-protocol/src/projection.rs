//! Projection: convert `Vec<Message>` → `Vec<ProjectedMessage>`.
//!
//! Used when restoring a session so consumers can render historical messages
//! without replaying the full agent event stream.
//!
//! AttemptCompletion tool results are promoted to assistant messages rather
//! than shown as tool output.

use std::collections::HashMap;

use loopal_message::{ContentBlock, Message, MessageRole};

use crate::projected::{ProjectedMessage, ProjectedToolCall};

/// Project a slice of Messages into ProjectedMessages.
pub fn project_messages(messages: &[Message]) -> Vec<ProjectedMessage> {
    let mut output: Vec<ProjectedMessage> = Vec::new();
    let mut tool_index: HashMap<String, (usize, usize, String)> = HashMap::new();

    for msg in messages {
        let role = role_str(&msg.role);
        let mut content_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<ProjectedToolCall> = Vec::new();
        let mut image_count: usize = 0;

        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => content_parts.push(text.clone()),
                ContentBlock::ToolUse { id, name, input } => {
                    let tc_idx = tool_calls.len();
                    tool_calls.push(ProjectedToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        summary: format!("{}({})", name, summarize_input(input)),
                        result: None,
                        is_error: false,
                        input: Some(input.clone()),
                        metadata: None,
                    });
                    tool_index.insert(id.clone(), (output.len(), tc_idx, name.clone()));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    is_completion,
                    ..
                } => {
                    back_patch(
                        &mut output,
                        &tool_index,
                        tool_use_id,
                        content,
                        *is_error,
                        *is_completion,
                    );
                }
                ContentBlock::Image { .. } => {
                    content_parts.push("[image]".to_string());
                    image_count += 1;
                }
                ContentBlock::Thinking { .. } => {}
                ContentBlock::ServerToolUse { id, name, input } => {
                    tool_calls.push(ProjectedToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        summary: format!("{}({})", name, summarize_input(input)),
                        result: None,
                        is_error: false,
                        input: Some(input.clone()),
                        metadata: None,
                    });
                }
                ContentBlock::ServerToolResult {
                    tool_use_id,
                    content,
                    ..
                } => {
                    if let Some(tc) = tool_calls.iter_mut().rev().find(|tc| tc.id == *tool_use_id) {
                        tc.result = Some(format_server_tool_content(content));
                    }
                }
            }
        }

        let content = content_parts.join("");
        if content.is_empty() && tool_calls.is_empty() {
            continue;
        }
        output.push(ProjectedMessage {
            role,
            content,
            tool_calls,
            image_count,
        });
    }

    output
}

/// Back-patch a ToolResult into the matching ProjectedToolCall.
fn back_patch(
    output: &mut Vec<ProjectedMessage>,
    index: &HashMap<String, (usize, usize, String)>,
    tool_use_id: &str,
    result: &str,
    is_error: bool,
    is_completion: bool,
) {
    let Some(&(di, ti, ref _name)) = index.get(tool_use_id) else {
        return;
    };
    if let Some(msg) = output.get_mut(di)
        && let Some(tc) = msg.tool_calls.get_mut(ti)
    {
        tc.is_error = is_error;
        if !is_completion {
            tc.result = Some(truncate_result(result));
        }
    }
    if is_completion {
        output.push(ProjectedMessage {
            role: "assistant".to_string(),
            content: result.to_string(),
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

fn truncate_result(s: &str) -> String {
    const MAX_LINES: usize = 200;
    const MAX_BYTES: usize = 10_000;
    let line_limited: String = s.lines().take(MAX_LINES).collect::<Vec<_>>().join("\n");
    if line_limited.len() <= MAX_BYTES {
        line_limited
    } else {
        let mut end = MAX_BYTES;
        while end > 0 && !line_limited.is_char_boundary(end) {
            end -= 1;
        }
        line_limited[..end].to_string()
    }
}

fn role_str(role: &MessageRole) -> String {
    match role {
        MessageRole::User => "user".to_string(),
        MessageRole::Assistant => "assistant".to_string(),
        MessageRole::System => "system".to_string(),
    }
}

/// Format server tool content for projection (simplified from session layer).
fn format_server_tool_content(content: &serde_json::Value) -> String {
    if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    content.to_string()
}
