use std::collections::HashSet;

use crate::token_counter::estimate_tokens;
use loopal_provider_api::{ContentBlock, Message, MessageRole};

const CAP_MAX_LINES: usize = 500;
const CAP_MAX_BYTES: usize = 20_000;

pub fn cap_tool_results(msg: &mut Message, max_tokens: u32) {
    if msg.role != MessageRole::User {
        return;
    }
    for block in &mut msg.content {
        if let ContentBlock::ToolResult {
            content, is_error, ..
        } = block
        {
            if *is_error {
                continue;
            }
            let tokens = estimate_tokens(content);
            if tokens > max_tokens {
                safe_truncate_tool_result(block, CAP_MAX_LINES, CAP_MAX_BYTES);
            }
        }
    }
}

pub fn cap_assistant_server_blocks(msg: &mut Message, max_tokens: u32) {
    if msg.role != MessageRole::Assistant {
        return;
    }
    // Strip orphaned pairs first — a truncated LLM response can leave a
    // ServerToolUse without its result; the API rejects that body.
    strip_orphaned_server_tool_blocks(msg);

    let has_server_blocks = msg
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::ServerToolResult { .. }));
    if !has_server_blocks {
        return;
    }
    let msg_tokens = msg.estimated_token_count();
    if msg_tokens <= max_tokens {
        return;
    }
    condense_server_blocks_in_message(msg);
}

fn strip_orphaned_server_tool_blocks(msg: &mut Message) {
    let result_ids: HashSet<String> = msg
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ServerToolResult { tool_use_id, .. } => Some(tool_use_id.clone()),
            _ => None,
        })
        .collect();

    let use_ids: HashSet<String> = msg
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ServerToolUse { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect();

    if result_ids.is_empty() && use_ids.is_empty() {
        return;
    }

    msg.content.retain(|b| match b {
        ContentBlock::ServerToolUse { id, .. } => result_ids.contains(id),
        ContentBlock::ServerToolResult { tool_use_id, .. } => use_ids.contains(tool_use_id),
        _ => true,
    });
}

fn condense_server_blocks_in_message(msg: &mut Message) {
    let mut replacements: Vec<(usize, ContentBlock)> = Vec::new();
    let mut removals: Vec<usize> = Vec::new();

    for (bi, block) in msg.content.iter().enumerate() {
        match block {
            ContentBlock::ServerToolUse { name, .. } => {
                replacements.push((
                    bi,
                    ContentBlock::Text {
                        text: server_tool_condensed_marker(name),
                    },
                ));
            }
            ContentBlock::ServerToolResult { .. } => {
                removals.push(bi);
            }
            _ => {}
        }
    }

    for (bi, replacement) in replacements {
        msg.content[bi] = replacement;
    }
    for bi in removals.into_iter().rev() {
        msg.content.remove(bi);
    }
}

pub fn condense_old_server_blocks(messages: &mut [Message]) {
    let last_assistant_idx = messages
        .iter()
        .rposition(|m| m.role == MessageRole::Assistant);

    for (i, msg) in messages.iter_mut().enumerate() {
        if msg.role != MessageRole::Assistant || Some(i) == last_assistant_idx {
            continue;
        }
        condense_server_blocks_in_message(msg);
    }
}

// Unified placeholder a condensed server-tool result collapses to. Shared by the
// ContentBlock-layer (wire Message) and AssistantOutput-layer (Turn) condensers.
pub(crate) fn server_tool_condensed_marker(name: &str) -> String {
    format!("[server tool '{name}' result condensed]")
}

// Fold one assistant response's server-tool pairs into text markers and drop the
// server_blocks (reasoning included — its web_search_call no longer exists). Shared
// by ingestion's defensive recovery and turn_degradation's old-turn budget trim.
pub(crate) fn condense_server_pairs_into_text(response: &mut loopal_turn::AssistantOutput) {
    use loopal_turn::{ServerBlock, TextBlock};
    if response.server_blocks.is_empty() {
        return;
    }
    for block in &response.server_blocks {
        if let ServerBlock::ToolPair(p) = block {
            response.text_blocks.push(TextBlock {
                text: server_tool_condensed_marker(&p.call.name),
            });
        }
    }
    response.server_blocks.clear();
}

// Defensive recovery for Anthropic ServerBlockError: clears server tool
// pairs across all turns and leaves a marker so next request validates.
pub(crate) fn condense_server_blocks_in_turns(turns: &mut [loopal_turn::Turn]) {
    use loopal_turn::TurnStep;
    for turn in turns {
        for step in &mut turn.body.steps {
            if let TurnStep::LlmCall { response, .. } = step {
                condense_server_pairs_into_text(response);
            }
        }
    }
}

// ServerToolResult / Text variants are intentionally untouched — only String
// content of ToolResult is safe to truncate (JSON body would corrupt).
pub fn safe_truncate_tool_result(block: &mut ContentBlock, max_lines: usize, max_bytes: usize) {
    let content = match block {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            if *is_error {
                return;
            }
            content
        }
        _ => return,
    };

    if content.len() <= max_bytes && content.lines().count() <= max_lines {
        return;
    }

    let original_bytes = content.len();
    let original_lines = content.lines().count();
    let truncated = loopal_tool_api::truncate_output(content, max_lines, max_bytes);
    let kept_bytes = truncated.len().min(original_bytes);
    *content = format!(
        "{truncated}\n[Truncated: kept {kept_bytes}/{original_bytes} bytes, \
         approx {max_lines}/{original_lines} lines]"
    );
}

#[cfg(test)]
mod tests;
