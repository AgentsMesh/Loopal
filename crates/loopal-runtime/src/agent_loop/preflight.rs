use loopal_context::compact_messages;
use loopal_context::compaction::{
    find_largest_tool_result, strip_old_thinking, truncate_block_content,
};
use loopal_context::token_counter::{estimate_messages_tokens, estimate_tokens};
use loopal_message::Message;
use tracing::{debug, info, warn};

use super::runner::AgentLoopRunner;

/// Max bytes to keep when truncating a ToolResult during preflight.
const PREFLIGHT_SUMMARY_MAX_BYTES: usize = 500;
/// Max lines to keep when truncating a ToolResult during preflight.
const PREFLIGHT_SUMMARY_MAX_LINES: usize = 20;
/// Minimum ToolResult size (bytes) worth truncating.
const MIN_TRUNCATABLE_BYTES: usize = 1_000;
/// Maximum truncation iterations to prevent infinite loops.
const MAX_ITERATIONS: usize = 20;

impl AgentLoopRunner {
    /// Pre-flight check on a working copy of messages.
    /// Ensures estimated messages + overhead fit within 95% of context window.
    /// Iteratively truncates the largest ToolResult blocks, then falls back
    /// to compact_messages. The caller's `messages` vec is mutated in place.
    pub fn preflight_check_on(&self, messages: &mut Vec<Message>) {
        // Strip old thinking blocks to save context tokens
        strip_old_thinking(messages);

        let system_tokens = estimate_tokens(&self.params.system_prompt);
        let tool_overhead: u32 = 2000;
        let budget = (self.model_config.max_context_tokens as f64 * 0.95) as u32;
        let overhead = system_tokens + tool_overhead;

        for iteration in 0..MAX_ITERATIONS {
            let msg_tokens = estimate_messages_tokens(messages);
            let total = msg_tokens + overhead;

            if total <= budget {
                debug!(
                    total,
                    budget,
                    messages = messages.len(),
                    "preflight: within budget"
                );
                return;
            }

            if let Some((mi, bi, size)) = find_largest_tool_result(messages) {
                if size < MIN_TRUNCATABLE_BYTES {
                    info!(
                        total,
                        budget, iteration, "preflight: no large ToolResults, emergency compact"
                    );
                    compact_messages(messages, 3);
                    return;
                }

                warn!(
                    total,
                    budget,
                    iteration,
                    msg_idx = mi,
                    block_idx = bi,
                    block_bytes = size,
                    "preflight: truncating largest ToolResult"
                );
                truncate_block_content(
                    &mut messages[mi].content[bi],
                    PREFLIGHT_SUMMARY_MAX_LINES,
                    PREFLIGHT_SUMMARY_MAX_BYTES,
                );
            } else {
                info!(
                    total,
                    budget, "preflight: no ToolResults, emergency compact"
                );
                compact_messages(messages, 3);
                return;
            }
        }

        warn!("preflight: max iterations reached, forcing compact");
        compact_messages(messages, 3);
    }
}
