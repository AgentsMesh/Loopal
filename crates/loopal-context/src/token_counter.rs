use loopal_types::message::Message;

/// Estimate token count: roughly 1 token per 4 characters.
pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() as u32) / 4
}

/// Estimate total tokens across a slice of messages.
/// Uses Message::estimated_token_count() which covers all ContentBlock variants
/// (Text, ToolUse, ToolResult, Image), not just text content.
pub fn estimate_messages_tokens(messages: &[Message]) -> u32 {
    messages
        .iter()
        .map(|m| m.estimated_token_count())
        .sum()
}
