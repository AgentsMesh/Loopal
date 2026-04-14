//! Fork context compression — prepare parent messages for child agents.

use crate::token_counter::estimate_messages_tokens;
use loopal_message::{ContentBlock, Message, MessageRole};

const FORK_MAX_TOKENS: u32 = 50_000;
const TOOL_RESULT_CAP: usize = 200;

/// Compress parent conversation for fork context.
///
/// Strips ephemeral blocks (thinking, image, server), truncates tool results,
/// drops the last incomplete assistant turn, and caps total tokens.
pub fn compress_for_fork(messages: &[Message]) -> Vec<Message> {
    if messages.is_empty() {
        return Vec::new();
    }

    let trimmed = strip_incomplete_tail(messages);
    let mut result: Vec<Message> = trimmed
        .iter()
        .map(compress_message)
        .filter(|m| !m.content.is_empty())
        .collect();

    // Drop oldest messages until within token budget (may trim to empty).
    let mut trimmed_budget = false;
    while !result.is_empty() && estimate_messages_tokens(&result) > FORK_MAX_TOKENS {
        result.remove(0);
        trimmed_budget = true;
    }
    // After budget trimming, ensure we start with a User message (API requirement).
    if trimmed_budget {
        while !result.is_empty() && result[0].role != MessageRole::User {
            result.remove(0);
        }
    }
    result
}

/// Boilerplate prepended to the child's prompt when fork context is injected.
pub const FORK_BOILERPLATE: &str = "\
STOP. READ THIS FIRST.\n\n\
You are a forked worker process. The conversation above is background \
context from your parent agent.\n\n\
RULES:\n\
1. Do NOT spawn sub-agents — execute directly with your tools.\n\
2. Stay within your assigned scope.\n\
3. Use tools silently, then report findings once at the end.\n\
4. Keep your report under 500 words. Be factual and concise.\n\
5. Your response MUST begin with \"Scope:\" — no preamble.\n\n\
Your task follows below.\n\n";

fn strip_incomplete_tail(messages: &[Message]) -> &[Message] {
    if let Some(last) = messages.last()
        && last.role == MessageRole::Assistant
        && has_tool_use(last)
    {
        return &messages[..messages.len() - 1];
    }
    messages
}

fn has_tool_use(msg: &Message) -> bool {
    msg.content
        .iter()
        .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
}

fn compress_message(msg: &Message) -> Message {
    let content: Vec<ContentBlock> = msg.content.iter().filter_map(compress_block).collect();
    Message {
        id: msg.id.clone(),
        role: msg.role.clone(),
        content,
    }
}

fn compress_block(block: &ContentBlock) -> Option<ContentBlock> {
    match block {
        ContentBlock::Thinking { .. }
        | ContentBlock::Image { .. }
        | ContentBlock::ServerToolUse { .. }
        | ContentBlock::ServerToolResult { .. } => None,
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            metadata,
        } => {
            let truncated = if content.len() > TOOL_RESULT_CAP {
                let boundary = floor_char_boundary(content, TOOL_RESULT_CAP);
                format!("{}…[truncated]", &content[..boundary])
            } else {
                content.clone()
            };
            Some(ContentBlock::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: truncated,
                is_error: *is_error,
                metadata: metadata.clone(),
            })
        }
        other => Some(other.clone()),
    }
}

fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
