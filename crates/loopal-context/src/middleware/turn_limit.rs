use async_trait::async_trait;
use loopal_types::error::LoopalError;
use loopal_types::middleware::{Middleware, MiddlewareContext};

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

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopalError> {
        if ctx.turn_count >= self.max_turns {
            return Err(LoopalError::Other(format!(
                "turn limit reached: {} >= {}",
                ctx.turn_count, self.max_turns
            )));
        }
        Ok(())
    }
}
