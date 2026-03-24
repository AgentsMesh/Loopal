//! LLM-based summarization for context compaction.

use loopal_error::LoopalError;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_provider_api::Provider;

use super::smart_compact_llm::call_summarization_llm;

/// Summarize old messages via LLM, returning the new compacted message list.
///
/// Splits `messages` at `messages.len() - keep_last`, summarizes the older portion,
/// and returns `[summary_msg, ack_msg, ...kept_messages]`.
///
/// Returns `Ok(Some(new_messages))` on success, `Ok(None)` if nothing to do,
/// `Err` if the LLM call failed.
pub async fn summarize_old_messages(
    messages: &[Message],
    provider: &dyn Provider,
    model: &str,
    keep_last: usize,
) -> Result<Option<Vec<Message>>, LoopalError> {
    if messages.len() <= keep_last {
        return Ok(None);
    }
    let split_at = messages.len() - keep_last;
    let old_messages = &messages[..split_at];
    if old_messages.is_empty() {
        return Ok(None);
    }

    let conversation_text = build_conversation_text(old_messages);
    let touched_files = extract_touched_files(old_messages);
    let summary_text = call_summarization_llm(provider, model, &conversation_text).await?;

    if summary_text.is_empty() {
        return Err(LoopalError::Provider(loopal_error::ProviderError::Api {
            status: 0,
            message: "empty summary response".to_string(),
        }));
    }

    tracing::info!(
        summary_len = summary_text.len(),
        old_messages = old_messages.len(),
        touched_files = touched_files.len(),
        "generated working state summary"
    );

    // Build summary with file list for rehydration
    let mut summary_body = format!(
        "[Working state summary of {} earlier messages]\n\n{}",
        old_messages.len(),
        summary_text
    );
    if !touched_files.is_empty() {
        summary_body.push_str("\n\n## Recently Touched Files\n");
        for file in &touched_files {
            summary_body.push_str(&format!("- {file}\n"));
        }
        summary_body.push_str("\nThese files may have changed. Re-read before editing.");
    }

    let summary_msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text { text: summary_body }],
    };
    let ack_msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::Text {
            text: "Understood. I'll continue from this working state.".to_string(),
        }],
    };

    let mut new_messages = vec![summary_msg, ack_msg];
    new_messages.extend_from_slice(&messages[split_at..]);

    Ok(Some(new_messages))
}

/// Build a text representation of messages for the summarization prompt.
fn build_conversation_text(messages: &[Message]) -> String {
    let mut text = String::new();
    for msg in messages {
        let role = match msg.role {
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::System => "System",
        };
        let content = msg.text_content();
        if !content.is_empty() {
            text.push_str(&format!("{role}: {content}\n\n"));
        }
        for block in &msg.content {
            match block {
                ContentBlock::ToolUse { name, input, .. } => {
                    // Include key params so LLM knows what was operated on
                    let args = extract_tool_args(name, input);
                    text.push_str(&format!("[Tool call: {name}({args})]\n"));
                }
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => {
                    let status = if *is_error { "error" } else { "ok" };
                    let preview = truncate_preview(content, 200);
                    text.push_str(&format!("[Tool result ({status}): {preview}]\n"));
                }
                ContentBlock::ServerToolUse { name, .. } => {
                    text.push_str(&format!("[Server tool: {name}]\n"));
                }
                ContentBlock::ServerToolResult { .. } => {
                    text.push_str("[Server tool result received]\n");
                }
                _ => {}
            }
        }
    }
    text
}

/// Truncate a string to `max_bytes` on a char boundary.
fn truncate_preview(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...[truncated]", &s[..end])
}

/// Extract key arguments from a tool call for the summarization prompt.
/// Preserves file paths and commands — the most important context for decisions.
fn extract_tool_args(name: &str, input: &serde_json::Value) -> String {
    match name {
        "Read" | "Write" | "Edit" | "MultiEdit" => input
            .get("file_path")
            .or_else(|| input.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|c| truncate_preview(c, 80))
            .unwrap_or_default(),
        "Grep" | "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| truncate_preview(p, 60))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Extract deduplicated file paths from ToolUse blocks (Read/Write/Edit/MultiEdit).
/// Used for rehydration hints in the summary.
fn extract_touched_files(messages: &[Message]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut files = Vec::new();
    for msg in messages {
        for block in &msg.content {
            if let ContentBlock::ToolUse { name, input, .. } = block {
                if matches!(name.as_str(), "Read" | "Write" | "Edit" | "MultiEdit") {
                    let path = input
                        .get("file_path")
                        .or_else(|| input.get("path"))
                        .and_then(|v| v.as_str());
                    if let Some(p) = path {
                        if seen.insert(p.to_string()) {
                            files.push(p.to_string());
                        }
                    }
                }
            }
        }
    }
    files
}
