//! Layered degradation pipeline — progressive content reduction.
//!
//! Runs synchronously after every message push. Each layer has an independent
//! trigger threshold and operates on the persistent message store (not a clone).
//!
//! | Layer | Trigger     | Operation                              |
//! |-------|-------------|----------------------------------------|
//! | 0     | Always      | Strip old thinking/server/image blocks |
//! | 1     | >60% budget | Truncate oversized old ToolResults     |
//!
//! Layer 2 (LLM summarization) lives in the runtime; it is the only path
//! that can free large amounts of tokens, and it works by anchoring a
//! `Marker::CompactBoundary` — not by dropping messages locally.

use crate::budget::ContextBudget;
use crate::compact_config::{
    LAYER1_TRIGGER_PERCENT, LAYER1_TRUNCATE_MAX_BYTES, LAYER1_TRUNCATE_MAX_LINES,
};
use crate::ingestion::{condense_old_server_blocks, safe_truncate_tool_result};
use crate::token_counter::estimate_messages_tokens;
use loopal_provider_api::{ContentBlock, Message, MessageRole};

pub fn run_sync_degradation(messages: &mut Vec<Message>, budget: &ContextBudget) {
    strip_ephemeral_blocks(messages);

    let tokens = estimate_messages_tokens(messages);
    if tokens > budget.message_budget * LAYER1_TRIGGER_PERCENT / 100 {
        truncate_oversized_results(messages, budget);
    }
}

fn strip_ephemeral_blocks(messages: &mut Vec<Message>) {
    condense_old_server_blocks(messages);

    let last_assistant_idx = messages
        .iter()
        .rposition(|m| m.role == MessageRole::Assistant);
    let preserve_images_from = messages.len().saturating_sub(2);

    for (i, msg) in messages.iter_mut().enumerate() {
        if msg.role == MessageRole::Assistant && Some(i) != last_assistant_idx {
            msg.content
                .retain(|b| !matches!(b, ContentBlock::Thinking { .. }));
        }
        if i < preserve_images_from {
            msg.content
                .retain(|b| !matches!(b, ContentBlock::Image { .. }));
        }
    }

    messages.retain(|m| m.role == MessageRole::System || !m.content.is_empty());
}

fn truncate_oversized_results(messages: &mut [Message], budget: &ContextBudget) {
    let threshold = budget.message_budget / 8;
    let recent_boundary = messages.len().saturating_sub(4);

    for (i, msg) in messages.iter_mut().enumerate() {
        if i >= recent_boundary {
            break;
        }
        if msg.role != MessageRole::User {
            continue;
        }
        for block in &mut msg.content {
            if let ContentBlock::ToolResult { content, .. } = block {
                let tokens = crate::token_counter::estimate_tokens(content);
                if tokens > threshold {
                    safe_truncate_tool_result(
                        block,
                        LAYER1_TRUNCATE_MAX_LINES,
                        LAYER1_TRUNCATE_MAX_BYTES,
                    );
                }
            }
        }
    }
}
