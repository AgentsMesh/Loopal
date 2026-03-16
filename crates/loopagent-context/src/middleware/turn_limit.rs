use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::middleware::{Middleware, MiddlewareContext};

/// Abort if turn count exceeds the configured maximum.
pub struct TurnLimit {
    pub max_turns: u32,
}

impl TurnLimit {
    pub fn new(max_turns: u32) -> Self {
        Self { max_turns }
    }
}

#[async_trait]
impl Middleware for TurnLimit {
    fn name(&self) -> &str {
        "turn_limit"
    }

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopAgentError> {
        if ctx.turn_count >= self.max_turns {
            return Err(LoopAgentError::Other(format!(
                "turn limit reached: {} >= {}",
                ctx.turn_count, self.max_turns
            )));
        }
        Ok(())
    }
}
