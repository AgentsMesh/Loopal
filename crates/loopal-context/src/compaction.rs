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
/// Also ensures every ServerToolUse has a matching ServerToolResult within the
/// same assistant message (both live in the same message, unlike client tools).
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

    // Pass 5: sanitize ServerToolUse/ServerToolResult pairs within each assistant message.
    // Both block types live in the same message (unlike client ToolUse/ToolResult).
    sanitize_server_tool_pairs(messages);

    // Pass 6: remove empty non-system messages
    messages.retain(|m| m.role == MessageRole::System || !m.content.is_empty());
}

/// Server-side tool name for code execution (mirrors provider constant).
const CODE_EXECUTION: &str = "code_execution";

/// Remove orphaned ServerToolUse / ServerToolResult blocks within each assistant message.
///
/// A ServerToolUse without a matching ServerToolResult (same message, matching id)
/// is dropped — this happens when the LLM response is truncated mid-server-tool.
/// Also strips code_execution blocks with empty input (parser bug artifact).
fn sanitize_server_tool_pairs(messages: &mut [Message]) {
    for msg in messages
        .iter_mut()
        .filter(|m| m.role == MessageRole::Assistant)
    {
        // Strip code_execution blocks with empty input (parser bug artifact).
        // Their orphaned results will be removed by the pair-matching below.
        msg.content.retain(|b| match b {
            ContentBlock::ServerToolUse { name, input, .. } => {
                !(name == CODE_EXECUTION && is_empty_json(input))
            }
            _ => true,
        });

        // Single pass: collect both use_ids and result_ids simultaneously.
        let mut result_ids = HashSet::new();
        let mut use_ids = HashSet::new();
        for b in &msg.content {
            match b {
                ContentBlock::ServerToolResult { tool_use_id, .. } => {
                    result_ids.insert(tool_use_id.clone());
                }
                ContentBlock::ServerToolUse { id, .. } => {
                    use_ids.insert(id.clone());
                }
                _ => {}
            }
        }

        msg.content.retain(|b| match b {
            ContentBlock::ServerToolUse { id, .. } => result_ids.contains(id),
            ContentBlock::ServerToolResult { tool_use_id, .. } => use_ids.contains(tool_use_id),
            _ => true,
        });
    }
}

fn is_empty_json(v: &serde_json::Value) -> bool {
    v.is_null() || v.as_object().is_some_and(|o| o.is_empty())
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
