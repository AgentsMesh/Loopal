use loopal_types::error::LoopalError;
use loopal_types::middleware::{Middleware, MiddlewareContext};

/// Pipeline that runs middleware in order.
pub struct ContextPipeline {
    middlewares: Vec<Box<dyn Middleware>>,
}

impl ContextPipeline {
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    pub fn add(&mut self, middleware: Box<dyn Middleware>) {
        self.middlewares.push(middleware);
    }

    /// Execute all middleware in order. Stops on first error.
    pub async fn execute(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopalError> {
        for mw in &self.middlewares {
            tracing::debug!(middleware = mw.name(), "executing middleware");
            mw.process(ctx).await?;
        }
        Ok(())
    }
}

impl Default for ContextPipeline {
    fn default() -> Self {
        Self::new()
    }
}
