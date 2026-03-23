use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_provider_api::{Middleware, MiddlewareContext};

use crate::compaction::{compact_messages, find_largest_tool_result, truncate_block_content};
use crate::token_counter::estimate_messages_tokens;

/// If estimated tokens exceed 80% of max, iteratively truncate the largest
/// ToolResult blocks. Falls back to `compact_messages` if no large blocks remain.
pub struct ContextGuard;

/// Minimum ToolResult size (bytes) worth truncating.
const MIN_TRUNCATABLE_BYTES: usize = 1_000;
/// When truncating a ToolResult, keep this many lines.
const SUMMARY_MAX_LINES: usize = 20;
/// When truncating a ToolResult, keep this many bytes.
const SUMMARY_MAX_BYTES: usize = 500;
/// Maximum truncation iterations to prevent infinite loops.
const MAX_ITERATIONS: usize = 20;

#[async_trait]
impl Middleware for ContextGuard {
    fn name(&self) -> &str {
        "context_guard"
    }

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopalError> {
        let threshold = (ctx.max_context_tokens as f64 * 0.8) as u32;

        for iteration in 0..MAX_ITERATIONS {
            let estimated = estimate_messages_tokens(&ctx.messages);
            if estimated <= threshold {
                tracing::debug!(
                    estimated,
                    threshold,
                    messages = ctx.messages.len(),
                    "context guard: within budget, no action"
                );
                return Ok(());
            }

            // Find the largest ToolResult and truncate it
            if let Some((mi, bi, size)) = find_largest_tool_result(&ctx.messages) {
                if size < MIN_TRUNCATABLE_BYTES {
                    // No large ToolResults left — fall back to message compaction
                    tracing::info!(
                        estimated,
                        threshold,
                        iteration,
                        "context guard: no large ToolResults, falling back to compact"
                    );
                    compact_messages(&mut ctx.messages, 5);
                    return Ok(());
                }

                tracing::info!(
                    estimated,
                    threshold,
                    iteration,
                    msg_idx = mi,
                    block_idx = bi,
                    block_bytes = size,
                    "context guard: truncating largest ToolResult"
                );
                truncate_block_content(
                    &mut ctx.messages[mi].content[bi],
                    SUMMARY_MAX_LINES,
                    SUMMARY_MAX_BYTES,
                );
            } else {
                // No ToolResults at all — compact messages
                tracing::info!(
                    estimated,
                    threshold,
                    "context guard: no ToolResults found, compacting messages"
                );
                compact_messages(&mut ctx.messages, 5);
                return Ok(());
            }
        }

        // Exhausted iterations — last-resort compaction
        tracing::warn!("context guard: max iterations reached, forcing compact");
        compact_messages(&mut ctx.messages, 5);
        Ok(())
    }
}
