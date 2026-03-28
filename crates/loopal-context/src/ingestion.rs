//! Ingestion gate — cap and condense content blocks before they enter the store.
//!
//! Type-safety rules:
//! - `ToolResult` (String content) → safe to truncate text
//! - `ServerToolResult` (JSON content) → only whole replacement with Text, NEVER truncate

use std::collections::HashSet;

use crate::token_counter::estimate_tokens;
use loopal_message::{ContentBlock, Message, MessageRole};

/// Max lines kept when truncating a single ToolResult.
const CAP_MAX_LINES: usize = 500;
/// Max bytes kept when truncating a single ToolResult.
const CAP_MAX_BYTES: usize = 20_000;

/// Cap oversized `ToolResult` blocks in a User message.
///
/// Any single `ToolResult` whose content exceeds `max_tokens` is truncated
/// to `CAP_MAX_LINES` / `CAP_MAX_BYTES`. Only operates on `ToolResult` (String);
/// `ServerToolResult` is never touched here.
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
                continue; // never truncate error messages
            }
            let tokens = estimate_tokens(content);
            if tokens > max_tokens {
                safe_truncate_tool_result(block, CAP_MAX_LINES, CAP_MAX_BYTES);
            }
        }
    }
}

/// Cap server blocks in a single assistant message if they exceed `max_tokens`.
///
/// Unlike `condense_old_server_blocks` (which skips the last assistant),
/// this targets a specific message — used at ingestion to prevent a single
/// turn's server blocks from consuming the entire budget.
pub fn cap_assistant_server_blocks(msg: &mut Message, max_tokens: u32) {
    if msg.role != MessageRole::Assistant {
        return;
    }
    // Strip orphaned pairs first — a truncated LLM response can leave
    // a ServerToolUse without its ServerToolResult, which the API rejects.
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
    // Message exceeds budget — condense server blocks unconditionally.
    // Even if text alone is large, stripping server blocks still frees tokens.
    condense_server_blocks_in_message(msg);
}

/// Strip `ServerToolUse` blocks that have no matching `ServerToolResult`,
/// and vice versa, within a single assistant message.
///
/// This happens when the LLM response is truncated (max_tokens) mid-server-tool.
/// Called at ingestion time to prevent broken pairs from entering the store.
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

    // Nothing to check if neither type is present
    if result_ids.is_empty() && use_ids.is_empty() {
        return;
    }

    msg.content.retain(|b| match b {
        ContentBlock::ServerToolUse { id, .. } => result_ids.contains(id),
        ContentBlock::ServerToolResult { tool_use_id, .. } => use_ids.contains(tool_use_id),
        _ => true,
    });
}

/// Condense server blocks within a single message (internal helper).
fn condense_server_blocks_in_message(msg: &mut Message) {
    let mut replacements: Vec<(usize, ContentBlock)> = Vec::new();
    let mut removals: Vec<usize> = Vec::new();

    for (bi, block) in msg.content.iter().enumerate() {
        match block {
            ContentBlock::ServerToolUse { name, .. } => {
                replacements.push((
                    bi,
                    ContentBlock::Text {
                        text: format!("[server tool '{name}' result condensed]"),
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

/// Condense `ServerToolUse` / `ServerToolResult` blocks in old assistant messages.
///
/// For each assistant message in `messages` (except the last), replaces:
/// - `ServerToolUse` → `Text("[server tool '{name}' was used]")`
/// - `ServerToolResult` → removed entirely
///
/// The assistant's `Text` block already contains the LLM's summary of the results,
/// so the raw server blocks are redundant after the first turn.
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

/// Condense server blocks in ALL assistant messages (including the last one).
/// Defensive recovery for API rejection of server blocks.
pub fn condense_all_server_blocks(messages: &mut [Message]) {
    for msg in messages.iter_mut() {
        if msg.role == MessageRole::Assistant {
            condense_server_blocks_in_message(msg);
        }
    }
}

/// Safely truncate a `ToolResult` block's String content.
///
/// Only operates on `ToolResult`. Returns immediately for any other variant,
/// including `ServerToolResult` — this is the type-safety guarantee that prevents
/// JSON corruption.
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
        _ => return, // ServerToolResult, Text, etc. — never truncate
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
