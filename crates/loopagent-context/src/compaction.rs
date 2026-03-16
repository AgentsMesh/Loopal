use loopagent_types::message::{Message, MessageRole};

/// Remove oldest messages, keeping the system message and the last `keep_last` messages.
pub fn compact_messages(messages: &mut Vec<Message>, keep_last: usize) {
    if messages.len() <= keep_last + 1 {
        return;
    }

    // Separate system messages (always at the front) from the rest
    let system_count = messages
        .iter()
        .take_while(|m| m.role == MessageRole::System)
        .count();

    let non_system_len = messages.len() - system_count;
    if non_system_len <= keep_last {
        return;
    }

    // Keep system messages + last `keep_last` non-system messages
    let remove_count = non_system_len - keep_last;
    messages.drain(system_count..system_count + remove_count);
}
