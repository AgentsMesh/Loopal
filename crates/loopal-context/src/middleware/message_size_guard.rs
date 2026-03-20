use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_provider_api::{Middleware, MiddlewareContext};

use crate::compaction::truncate_block_content;
use crate::token_counter::estimate_message_tokens;

/// Truncate any single message that exceeds 25% of the context window.
/// Targets the largest ToolResult block within oversized messages.
pub struct MessageSizeGuard;

/// Maximum lines to keep when truncating an oversized ToolResult.
const TRUNCATED_MAX_LINES: usize = 50;
/// Maximum bytes to keep when truncating an oversized ToolResult.
const TRUNCATED_MAX_BYTES: usize = 2_000;

#[async_trait]
impl Middleware for MessageSizeGuard {
    fn name(&self) -> &str {
        "message_size_guard"
    }

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopalError> {
        let threshold = ctx.max_context_tokens / 4;
        let mut truncated_count = 0u32;

        for msg in ctx.messages.iter_mut() {
            let msg_tokens = estimate_message_tokens(msg);
            if msg_tokens <= threshold {
                continue;
            }

            // Find the largest ToolResult block in this message and truncate it
            let largest = msg.content.iter().enumerate().max_by_key(|(_, block)| {
                if let loopal_message::ContentBlock::ToolResult { content, .. } = block {
                    content.len()
                } else {
                    0
                }
            });

            if let Some((idx, _)) = largest {
                tracing::info!(
                    msg_tokens,
                    threshold,
                    block_idx = idx,
                    "message_size_guard: truncating oversized ToolResult"
                );
                truncate_block_content(&mut msg.content[idx], TRUNCATED_MAX_LINES, TRUNCATED_MAX_BYTES);
                truncated_count += 1;
            }
        }

        if truncated_count == 0 {
            tracing::debug!(
                threshold, messages = ctx.messages.len(),
                "message_size_guard: all messages within budget, no action"
            );
        }

        Ok(())
    }
}
