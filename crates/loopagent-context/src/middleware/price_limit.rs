use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::middleware::{Middleware, MiddlewareContext};

/// Abort if total cost exceeds the configured maximum.
pub struct PriceLimit {
    pub max_cost: f64,
}

impl PriceLimit {
    pub fn new(max_cost: f64) -> Self {
        Self { max_cost }
    }
}

#[async_trait]
impl Middleware for PriceLimit {
    fn name(&self) -> &str {
        "price_limit"
    }

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopAgentError> {
        if ctx.total_cost >= self.max_cost {
            return Err(LoopAgentError::Other(format!(
                "price limit reached: ${:.4} >= ${:.4}",
                ctx.total_cost, self.max_cost
            )));
        }
        Ok(())
    }
}
