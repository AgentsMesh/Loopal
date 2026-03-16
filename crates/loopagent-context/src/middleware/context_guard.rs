use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::middleware::{Middleware, MiddlewareContext};

use crate::compaction::compact_messages;
use crate::token_counter::estimate_messages_tokens;

/// If estimated tokens exceed 80% of max, trigger compaction.
pub struct ContextGuard;

#[async_trait]
impl Middleware for ContextGuard {
    fn name(&self) -> &str {
        "context_guard"
    }

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopAgentError> {
        let estimated = estimate_messages_tokens(&ctx.messages);
        let threshold = (ctx.max_context_tokens as f64 * 0.8) as u32;

        if estimated > threshold {
            tracing::info!(
                estimated,
                threshold,
                "context guard triggered, compacting messages"
            );
            compact_messages(&mut ctx.messages, 10);
        }

        Ok(())
    }
}
