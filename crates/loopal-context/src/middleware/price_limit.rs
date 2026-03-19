use async_trait::async_trait;
use loopal_types::error::LoopalError;
use loopal_types::middleware::{Middleware, MiddlewareContext};

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

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopalError> {
        if ctx.total_cost >= self.max_cost {
            return Err(LoopalError::Other(format!(
                "price limit reached: ${:.4} >= ${:.4}",
                ctx.total_cost, self.max_cost
            )));
        }
        Ok(())
    }
}
