use std::collections::HashSet;

use loopal_message::{ContentBlock, Message, MessageRole};

/// Remove oldest messages, keeping the system message and the last `keep_last` messages.
/// Post-processes with `sanitize_tool_pairs` to fix any broken tool_use/tool_result references.
pub fn compact_messages(messages: &mut Vec<Message>, keep_last: usize) {
    if messages.len() <= keep_last + 1 {
        return;
    }

    let system_count = messages
        .iter()
        .take_while(|m| m.role == MessageRole::System)
        .count();

    let non_system_len = messages.len() - system_count;
    if non_system_len <= keep_last {
        return;
    }

    let remove_count = non_system_len - keep_last;
    messages.drain(system_count..system_count + remove_count);

    sanitize_tool_pairs(messages);
}

/// Remove orphaned tool_use/tool_result blocks after compaction.
///
/// Ensures every ToolResult references an existing ToolUse (and vice versa).
/// Removes empty messages that result from block removal.
pub fn sanitize_tool_pairs(messages: &mut Vec<Message>) {
    // Pass 1: collect all ToolUse ids from assistant messages
    let tool_use_ids: HashSet<String> = messages
        .iter()
        .filter(|m| m.role == MessageRole::Assistant)
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolUse { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect();

    // Pass 2: remove orphaned ToolResult blocks
    for msg in messages.iter_mut().filter(|m| m.role == MessageRole::User) {
        msg.content.retain(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => tool_use_ids.contains(tool_use_id),
            _ => true,
        });
    }

    // Pass 3: collect all ToolResult tool_use_ids
    let tool_result_ids: HashSet<String> = messages
        .iter()
        .filter(|m| m.role == MessageRole::User)
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.clone()),
            _ => None,
        })
        .collect();

    // Pass 4: remove orphaned ToolUse blocks
    for msg in messages
        .iter_mut()
        .filter(|m| m.role == MessageRole::Assistant)
    {
        msg.content.retain(|b| match b {
            ContentBlock::ToolUse { id, .. } => tool_result_ids.contains(id),
            _ => true,
        });
    }

    // Pass 5: remove empty non-system messages
    messages.retain(|m| m.role == MessageRole::System || !m.content.is_empty());
}

/// Find the largest ToolResult or ServerToolResult content block across all messages.
/// Returns `(message_index, block_index, byte_size)`.
pub fn find_largest_result_block(messages: &[Message]) -> Option<(usize, usize, usize)> {
    let mut best: Option<(usize, usize, usize)> = None;
    for (mi, msg) in messages.iter().enumerate() {
        for (bi, block) in msg.content.iter().enumerate() {
            let size = match block {
                ContentBlock::ToolResult { content, .. } => content.len(),
                ContentBlock::ServerToolResult { content, .. } => content.to_string().len(),
                _ => continue,
            };
            if best.is_none_or(|(_, _, s)| size > s) {
                best = Some((mi, bi, size));
            }
        }
    }
    best
}

/// Truncate a ToolResult or ServerToolResult content block in place.
pub fn truncate_block_content(block: &mut ContentBlock, max_lines: usize, max_bytes: usize) {
    let text = match block {
        ContentBlock::ToolResult { content, .. } => content,
        ContentBlock::ServerToolResult { content, .. } => {
            // Convert JSON to string for truncation
            let s = content.to_string();
            *content = serde_json::Value::String(s);
            match content {
                serde_json::Value::String(s) => s,
                _ => unreachable!(),
            }
        }
        _ => return,
    };
    if text.len() <= max_bytes && text.lines().count() <= max_lines {
        return;
    }

    let original_bytes = text.len();
    let original_lines = text.lines().count();
    let mut result = String::new();
    let mut byte_count = 0;

    for (i, line) in text.lines().enumerate() {
        let line_bytes = line.len() + 1;
        if i >= max_lines || byte_count + line_bytes > max_bytes {
            break;
        }
        if i > 0 {
            result.push('\n');
        }
        result.push_str(line);
        byte_count += line_bytes;
    }

    result.push_str(&format!(
        "\n\n[Truncated: kept {byte_count}/{original_bytes} bytes, \
         {}/{original_lines} lines]",
        result.lines().count()
    ));
    *text = result;
}

/// Strip `ContentBlock::Thinking` blocks from all assistant messages except the last one.
/// The last assistant message's thinking block must be preserved for Anthropic signature
/// verification in multi-turn conversations. Older thinking blocks waste context tokens.
pub fn strip_old_thinking(messages: &mut [Message]) {
    let last_assistant_idx = messages
        .iter()
        .rposition(|m| m.role == MessageRole::Assistant);

    for (i, msg) in messages.iter_mut().enumerate() {
        if msg.role != MessageRole::Assistant {
            continue;
        }
        if Some(i) == last_assistant_idx {
            continue;
        }
        msg.content
            .retain(|block| !matches!(block, ContentBlock::Thinking { .. }));
    }
}

/// Strip server tool blocks (ServerToolUse + ServerToolResult) from all assistant messages
/// except the last one. Old search results waste context tokens — the text response already
/// summarized the key findings.
pub fn strip_old_server_tool_content(messages: &mut [Message]) {
    let last_assistant_idx = messages
        .iter()
        .rposition(|m| m.role == MessageRole::Assistant);

    for (i, msg) in messages.iter_mut().enumerate() {
        if msg.role != MessageRole::Assistant || Some(i) == last_assistant_idx {
            continue;
        }
        msg.content.retain(|block| {
            !matches!(
                block,
                ContentBlock::ServerToolUse { .. } | ContentBlock::ServerToolResult { .. }
            )
        });
    }
}

/// Strip `ContentBlock::Image` blocks from all messages except those in the last
/// user-assistant exchange. Old screenshots waste ~1000 tokens each.
pub fn strip_old_images(messages: &mut [Message]) {
    if messages.len() <= 2 {
        return;
    }
    // Preserve images only in the last 2 messages (last user + last assistant)
    let preserve_from = messages.len().saturating_sub(2);
    for (i, msg) in messages.iter_mut().enumerate() {
        if i >= preserve_from {
            continue;
        }
        msg.content
            .retain(|block| !matches!(block, ContentBlock::Image { .. }));
    }
}
