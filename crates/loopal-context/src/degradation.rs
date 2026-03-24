//! Layered degradation pipeline — progressive content reduction.
//!
//! Runs synchronously after every message push. Each layer has an independent
//! trigger threshold and operates on the persistent message store (not a clone).
//!
//! | Layer | Trigger     | Operation                              |
//! |-------|-------------|----------------------------------------|
//! | 0     | Always      | Strip old thinking/server/image blocks |
//! | 1     | >60% budget | Truncate oversized old ToolResults     |
//! | 3     | >90% budget | Emergency: summarize + drop oldest     |
//!
//! Layer 2 (LLM summarization at >75%) is async and driven by the runtime.

use crate::budget::ContextBudget;
use crate::ingestion::{condense_old_server_blocks, safe_truncate_tool_result};
use crate::token_counter::estimate_messages_tokens;
use loopal_message::{ContentBlock, Message, MessageRole};

/// Max lines for Layer-1 truncation of oversized old results.
const LAYER1_MAX_LINES: usize = 200;
/// Max bytes for Layer-1 truncation of oversized old results.
const LAYER1_MAX_BYTES: usize = 8_000;
/// Max lines for emergency (Layer-3) truncation.
const EMERGENCY_MAX_LINES: usize = 30;
/// Max bytes for emergency truncation.
const EMERGENCY_MAX_BYTES: usize = 1_000;

/// Run synchronous degradation layers 0, 1, and (if needed) 3.
///
/// Called after every message push to enforce the budget invariant.
pub fn run_sync_degradation(messages: &mut Vec<Message>, budget: &ContextBudget) {
    // Layer 0: always strip zero-value content
    strip_ephemeral_blocks(messages);

    let tokens = estimate_messages_tokens(messages);

    // Layer 1: truncate oversized old ToolResults when >60% budget
    if tokens > budget.message_budget * 60 / 100 {
        truncate_oversized_results(messages, budget);
    }

    // Layer 3: emergency degradation when >90% budget
    let tokens = estimate_messages_tokens(messages);
    if tokens > budget.message_budget * 90 / 100 {
        emergency_degrade(messages);
    }
}

/// Layer 0: Strip ephemeral blocks from non-recent messages.
///
/// Consolidates three previously separate operations into one pass:
/// - Old thinking blocks (preserve last assistant for signature verification)
/// - Old server tool blocks (replace ServerToolUse with Text, remove ServerToolResult)
/// - Old image blocks (preserve last 2 messages)
fn strip_ephemeral_blocks(messages: &mut Vec<Message>) {
    // Delegate to ingestion's condense (handles ServerToolUse/Result)
    condense_old_server_blocks(messages);

    let last_assistant_idx = messages
        .iter()
        .rposition(|m| m.role == MessageRole::Assistant);
    let preserve_images_from = messages.len().saturating_sub(2);

    for (i, msg) in messages.iter_mut().enumerate() {
        // Strip old thinking (preserve last assistant's for Anthropic signature)
        if msg.role == MessageRole::Assistant && Some(i) != last_assistant_idx {
            msg.content
                .retain(|b| !matches!(b, ContentBlock::Thinking { .. }));
        }
        // Strip old images (preserve last 2 messages)
        if i < preserve_images_from {
            msg.content
                .retain(|b| !matches!(b, ContentBlock::Image { .. }));
        }
    }

    // Remove messages that became empty after stripping
    messages.retain(|m| m.role == MessageRole::System || !m.content.is_empty());
}

/// Layer 1: Truncate oversized ToolResult blocks in old messages.
///
/// "Old" = all messages except the last 4 (2 turns of assistant+user pairs).
/// Only operates on ToolResult (String), never ServerToolResult.
fn truncate_oversized_results(messages: &mut [Message], budget: &ContextBudget) {
    let threshold = budget.message_budget / 8;
    let recent_boundary = messages.len().saturating_sub(4);

    for (i, msg) in messages.iter_mut().enumerate() {
        if i >= recent_boundary {
            break; // don't touch recent messages
        }
        if msg.role != MessageRole::User {
            continue;
        }
        for block in &mut msg.content {
            if let ContentBlock::ToolResult { content, .. } = block {
                let tokens = crate::token_counter::estimate_tokens(content);
                if tokens > threshold {
                    safe_truncate_tool_result(block, LAYER1_MAX_LINES, LAYER1_MAX_BYTES);
                }
            }
        }
    }
}

/// Layer 3: Emergency degradation when >90% budget.
///
/// 1. Aggressively truncate ALL old ToolResults to minimal summaries
/// 2. If still over budget, drop oldest message pairs
pub fn emergency_degrade(messages: &mut [Message]) {
    let recent = messages.len().saturating_sub(4);
    // Phase 1: aggressively truncate all old ToolResults
    for (i, msg) in messages.iter_mut().enumerate() {
        if i >= recent {
            break;
        }
        if msg.role != MessageRole::User {
            continue;
        }
        for block in &mut msg.content {
            safe_truncate_tool_result(block, EMERGENCY_MAX_LINES, EMERGENCY_MAX_BYTES);
        }
    }
}

/// Drop the oldest non-system message group to free tokens.
///
/// A "group" is an assistant message + the following user message (tool results).
/// Returns the number of messages removed, or 0 if nothing to drop.
pub fn drop_oldest_group(messages: &mut Vec<Message>) -> usize {
    let system_count = messages
        .iter()
        .take_while(|m| m.role == MessageRole::System)
        .count();

    // Need at least 4 non-system messages (keep the last group)
    let non_system = messages.len() - system_count;
    if non_system <= 4 {
        return 0;
    }

    // Find the first non-system message group (assistant + optional user)
    let start = system_count;
    let mut end = start + 1;
    if end < messages.len() && messages[end].role == MessageRole::User {
        end += 1;
    }

    let removed = end - start;
    messages.drain(start..end);
    removed
}
