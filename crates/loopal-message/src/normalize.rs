use crate::message::{ContentBlock, Message, MessageRole};

/// Normalize messages to ensure they alternate between user and assistant roles.
/// Consecutive messages with the same role are merged by concatenating their content blocks.
/// System messages are filtered out (they belong in the separate system_prompt parameter,
/// not in the messages array per the Anthropic/Google APIs).
pub fn normalize_messages(messages: &[Message]) -> Vec<Message> {
    let mut result: Vec<Message> = Vec::new();

    for msg in messages {
        // Filter out system messages entirely — Anthropic API requires system
        // prompt to be passed separately, not as a message in the array.
        if msg.role == MessageRole::System {
            continue;
        }

        if let Some(last) = result.last_mut()
            && last.role == msg.role
        {
            // Merge content blocks
            last.content.extend(msg.content.clone());
            continue;
        }

        result.push(msg.clone());
    }

    // Defensive: ensure ToolResult blocks come before Text blocks in User messages.
    // The Anthropic API requires tool_result to appear first when responding to
    // a tool_use. Merging consecutive User messages can violate this ordering.
    for msg in &mut result {
        if msg.role == MessageRole::User {
            stabilize_user_block_order(&mut msg.content);
        }
    }

    result
}

/// Reorder blocks in a User message so ToolResult blocks come before Text blocks.
/// Preserves relative order within each group (stable partition).
fn stabilize_user_block_order(blocks: &mut Vec<ContentBlock>) {
    if blocks.len() < 2 {
        return;
    }
    let has_tool_result = blocks.iter().any(|b| matches!(b, ContentBlock::ToolResult { .. }));
    let has_text_before = blocks
        .iter()
        .take_while(|b| !matches!(b, ContentBlock::ToolResult { .. }))
        .any(|b| matches!(b, ContentBlock::Text { .. }));
    if !has_tool_result || !has_text_before {
        return;
    }
    // Stable partition: ToolResult first, then everything else
    let mut tool_results = Vec::new();
    let mut rest = Vec::new();
    for b in blocks.drain(..) {
        if matches!(b, ContentBlock::ToolResult { .. }) {
            tool_results.push(b);
        } else {
            rest.push(b);
        }
    }
    blocks.extend(tool_results);
    blocks.extend(rest);
}
