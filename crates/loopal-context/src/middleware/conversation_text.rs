use loopal_message::{ContentBlock, Message, MessageRole};

const TOOL_RESULT_PREVIEW_BYTES: usize = 200;
const BASH_COMMAND_PREVIEW_BYTES: usize = 80;
const GREP_PATTERN_PREVIEW_BYTES: usize = 60;

pub(super) fn build_conversation_text(messages: &[Message]) -> String {
    // Roughly 200 bytes/message in practice (role tag + 1-2 short blocks).
    // Over-allocating here avoids a chain of doublings on long histories.
    let mut text = String::with_capacity(messages.len().saturating_mul(200));
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
                    let args = extract_tool_args(name, input);
                    text.push_str(&format!("[Tool call: {name}({args})]\n"));
                }
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => {
                    let status = if *is_error { "error" } else { "ok" };
                    let preview = truncate_preview(content, TOOL_RESULT_PREVIEW_BYTES);
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
            .map(|c| truncate_preview(c, BASH_COMMAND_PREVIEW_BYTES))
            .unwrap_or_default(),
        "Grep" | "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| truncate_preview(p, GREP_PATTERN_PREVIEW_BYTES))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_message::Message;

    #[test]
    fn empty_messages_render_empty() {
        assert_eq!(build_conversation_text(&[]), "");
    }

    #[test]
    fn user_message_text_is_rendered() {
        let m = Message::user("hello world");
        let out = build_conversation_text(&[m]);
        assert!(out.contains("User: hello world"));
    }

    #[test]
    fn tool_call_shows_name_and_args() {
        let m = Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "t1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "/x.rs"}),
            }],
            origin: None,
        };
        let out = build_conversation_text(&[m]);
        assert!(out.contains("[Tool call: Read(/x.rs)]"));
    }

    #[test]
    fn tool_result_truncated_to_preview_size() {
        let big = "x".repeat(500);
        let m = Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "t1".into(),
                content: big,
                images: Vec::new(),
                is_error: false,
                metadata: None,
            }],
            origin: None,
        };
        let out = build_conversation_text(&[m]);
        assert!(out.contains("[truncated]"));
    }

    #[test]
    fn truncate_preview_respects_char_boundary() {
        let out = truncate_preview("你好啊", 4);
        assert!(out.starts_with("你"));
    }
}
