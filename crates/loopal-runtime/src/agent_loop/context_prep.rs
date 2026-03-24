//! Ephemeral context preparation — runs every LLM call on a clone.
//!
//! Lightweight mechanical operations: strip old thinking/server blocks,
//! then safety-check that the working copy fits within the context window.
//! This is the ONLY ephemeral step — everything else is persistent.

use loopal_context::budget::ContextBudget;
use loopal_context::compaction::{
    compact_messages, find_largest_result_block, strip_old_images, strip_old_server_tool_content,
    strip_old_thinking, truncate_block_content,
};
use loopal_context::token_counter::{estimate_message_tokens, estimate_messages_tokens};
use loopal_message::Message;
use tracing::{debug, info, warn};

use super::runner::AgentLoopRunner;

/// Max bytes for per-message oversized block truncation.
const MSG_GUARD_MAX_BYTES: usize = 2_000;
/// Max lines for per-message oversized block truncation.
const MSG_GUARD_MAX_LINES: usize = 50;
/// Max bytes for safety-net truncation (tighter).
const SAFETY_MAX_BYTES: usize = 500;
/// Max lines for safety-net truncation.
const SAFETY_MAX_LINES: usize = 20;
/// Minimum block size (bytes) worth truncating.
const MIN_TRUNCATABLE_BYTES: usize = 1_000;
/// Maximum truncation iterations.
const MAX_ITERATIONS: usize = 20;
/// Emergency compact keep count (absolute last resort).
const EMERGENCY_KEEP: usize = 3;

impl AgentLoopRunner {
    /// Prepare a working copy of messages for the LLM call.
    ///
    /// 1. Clone params.messages
    /// 2. Strip old thinking blocks (preserve last for Anthropic signature)
    /// 3. Strip old server tool blocks (search results from old turns)
    /// 4. Truncate per-message oversized result blocks (>25% of budget)
    /// 5. Safety net: if still over 95% of budget, iteratively truncate
    pub fn prepare_llm_context(&self) -> Vec<Message> {
        let mut working = self.params.messages.clone();

        // Content reduction — zero-value blocks in non-recent messages
        strip_old_thinking(&mut working);
        strip_old_server_tool_content(&mut working);
        strip_old_images(&mut working);

        let tool_defs = self.params.kernel.tool_definitions();
        let tool_tokens = ContextBudget::estimate_tool_tokens(&tool_defs);
        let budget = ContextBudget::calculate(
            self.model_config.max_context_tokens,
            &self.params.system_prompt,
            tool_tokens,
            self.model_config.max_output_tokens,
        );

        // Per-message guard: truncate any single message exceeding 25% of budget
        self.truncate_oversized_messages(&mut working, &budget);

        // Global safety net
        self.safety_truncate(&mut working, &budget);
        working
    }

    /// Truncate the largest result block in any message that exceeds 25% of budget.
    fn truncate_oversized_messages(&self, messages: &mut [Message], budget: &ContextBudget) {
        let threshold = budget.message_budget / 4;
        for msg in messages.iter_mut() {
            let msg_tokens = estimate_message_tokens(msg);
            if msg_tokens <= threshold {
                continue;
            }
            if let Some((idx, _)) = find_largest_block_in_message(msg) {
                info!(msg_tokens, threshold, "truncating oversized message block");
                truncate_block_content(
                    &mut msg.content[idx],
                    MSG_GUARD_MAX_LINES,
                    MSG_GUARD_MAX_BYTES,
                );
            }
        }
    }

    /// Iteratively truncate the largest result blocks until within budget.
    /// Last resort: compact_messages if no truncatable blocks remain.
    fn safety_truncate(&self, messages: &mut Vec<Message>, budget: &ContextBudget) {
        for iteration in 0..MAX_ITERATIONS {
            let msg_tokens = estimate_messages_tokens(messages);
            if !budget.needs_emergency(msg_tokens) {
                debug!(
                    msg_tokens,
                    budget = budget.message_budget,
                    "context prep: within budget"
                );
                return;
            }

            if let Some((mi, bi, size)) = find_largest_result_block(messages) {
                if size < MIN_TRUNCATABLE_BYTES {
                    info!(
                        iteration,
                        "context prep: no large blocks, emergency compact"
                    );
                    compact_messages(messages, EMERGENCY_KEEP);
                    return;
                }
                info!(
                    iteration,
                    msg_idx = mi,
                    block_bytes = size,
                    "context prep: truncating"
                );
                truncate_block_content(
                    &mut messages[mi].content[bi],
                    SAFETY_MAX_LINES,
                    SAFETY_MAX_BYTES,
                );
            } else {
                info!("context prep: no result blocks, emergency compact");
                compact_messages(messages, EMERGENCY_KEEP);
                return;
            }
        }

        warn!("context prep: max iterations reached, forcing compact");
        compact_messages(messages, EMERGENCY_KEEP);
    }
}

/// Find the largest result block within a single message. Returns (block_index, byte_size).
fn find_largest_block_in_message(msg: &Message) -> Option<(usize, usize)> {
    use loopal_message::ContentBlock;
    msg.content
        .iter()
        .enumerate()
        .filter_map(|(i, block)| {
            let size = match block {
                ContentBlock::ToolResult { content, .. } => content.len(),
                ContentBlock::ServerToolResult { content, .. } => content.to_string().len(),
                _ => return None,
            };
            Some((i, size))
        })
        .max_by_key(|(_, size)| *size)
}
