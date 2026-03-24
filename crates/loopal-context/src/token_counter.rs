use loopal_message::{ContentBlock, Message};
use std::sync::LazyLock;
use tiktoken_rs::CoreBPE;

/// Global BPE encoder singleton (cl100k_base, closest to Claude's tokenizer).
static BPE: LazyLock<CoreBPE> = LazyLock::new(|| {
    tiktoken_rs::cl100k_base().expect("failed to initialize cl100k_base BPE encoder")
});

/// Count tokens in a text string using BPE encoding.
pub fn estimate_tokens(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    BPE.encode_with_special_tokens(text).len() as u32
}

/// Count tokens for a single message across all content blocks.
pub fn estimate_message_tokens(msg: &Message) -> u32 {
    let content_tokens: u32 = msg
        .content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => estimate_tokens(text),
            ContentBlock::ToolUse { input, .. } => estimate_tokens(&input.to_string()),
            ContentBlock::ToolResult { content, .. } => estimate_tokens(content),
            ContentBlock::Image { .. } => 1000, // fixed estimate for images
            ContentBlock::Thinking { thinking, .. } => estimate_tokens(thinking),
            ContentBlock::ServerToolUse { input, .. } => estimate_tokens(&input.to_string()),
            ContentBlock::ServerToolResult { content, .. } => estimate_tokens(&content.to_string()),
        })
        .sum();
    // +4 for role/message framing overhead
    content_tokens + 4
}

/// Count total tokens across a slice of messages.
pub fn estimate_messages_tokens(messages: &[Message]) -> u32 {
    messages.iter().map(estimate_message_tokens).sum()
}
