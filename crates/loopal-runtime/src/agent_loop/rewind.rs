//! Turn boundary detection for the rewind feature.
//!
//! A "turn" starts at a User message that contains at least one `ContentBlock::Text`.
//! Pure `ToolResult`-only User messages are continuations, not turn starts.

use loopal_message::{ContentBlock, Message, MessageRole};

/// Detect turn boundaries in the message list.
///
/// Returns indices of messages that start a new user turn (User messages
/// containing at least one `Text` content block). Indices are sorted ascending.
pub fn detect_turn_boundaries(messages: &[Message]) -> Vec<usize> {
    messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == MessageRole::User && has_text_block(m))
        .map(|(i, _)| i)
        .collect()
}

fn has_text_block(msg: &Message) -> bool {
    msg.content
        .iter()
        .any(|b| matches!(b, ContentBlock::Text { .. }))
}

/// Build a preview string from a user message (truncated to `max_len` chars).
pub fn turn_preview(msg: &Message, max_len: usize) -> String {
    let text = msg.text_content();
    if text.chars().count() <= max_len {
        text
    } else {
        let truncated: String = text.chars().take(max_len).collect();
        format!("{truncated}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_boundaries_skips_tool_result_only() {
        let messages = vec![
            Message::user("hello"),
            Message::assistant("hi"),
            Message {
                id: None,
                role: MessageRole::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "t1".into(),
                    content: "ok".into(),
                    is_error: false,
                    metadata: None,
                }],
            },
            Message::user("second turn"),
        ];
        let boundaries = detect_turn_boundaries(&messages);
        assert_eq!(boundaries, vec![0, 3]);
    }
}
