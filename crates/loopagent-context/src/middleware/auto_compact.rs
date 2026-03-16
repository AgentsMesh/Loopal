use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::middleware::{Middleware, MiddlewareContext};

use crate::compaction::compact_messages;
use crate::token_counter::estimate_messages_tokens;

/// Automatically compact when context is too large (exceeds max_context_tokens).
pub struct AutoCompact {
    pub keep_last: usize,
}

impl AutoCompact {
    pub fn new(keep_last: usize) -> Self {
        Self { keep_last }
    }
}

#[async_trait]
impl Middleware for AutoCompact {
    fn name(&self) -> &str {
        "auto_compact"
    }

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopAgentError> {
        let estimated = estimate_messages_tokens(&ctx.messages);

        if estimated > ctx.max_context_tokens {
            tracing::info!(
                estimated,
                max = ctx.max_context_tokens,
                "auto-compacting messages"
            );
            compact_messages(&mut ctx.messages, self.keep_last);
        }

        Ok(())
    }
}
