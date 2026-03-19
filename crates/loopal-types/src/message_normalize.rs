use crate::message::{Message, MessageRole};

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
            && last.role == msg.role {
                // Merge content blocks
                last.content.extend(msg.content.clone());
                continue;
            }

        result.push(msg.clone());
    }

    result
}
