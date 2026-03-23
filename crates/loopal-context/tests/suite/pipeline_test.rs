use async_trait::async_trait;
use loopal_context::ContextPipeline;
use loopal_error::LoopalError;
use loopal_provider_api::{Middleware, MiddlewareContext};

struct AppendMiddleware {
    suffix: String,
}

#[async_trait]
impl Middleware for AppendMiddleware {
    fn name(&self) -> &str {
        "append"
    }

    async fn process(&self, ctx: &mut MiddlewareContext) -> Result<(), LoopalError> {
        ctx.system_prompt.push_str(&self.suffix);
        Ok(())
    }
}

struct FailMiddleware;

#[async_trait]
impl Middleware for FailMiddleware {
    fn name(&self) -> &str {
        "fail"
    }

    async fn process(&self, _ctx: &mut MiddlewareContext) -> Result<(), LoopalError> {
        Err(LoopalError::Other("fail".into()))
    }
}

fn make_ctx() -> MiddlewareContext {
    MiddlewareContext {
        messages: vec![],
        system_prompt: String::new(),
        model: "test".into(),
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cost: 0.0,
        max_context_tokens: 100_000,
        compact_model: None,
        summarization_provider: None,
    }
}

#[tokio::test]
async fn test_pipeline_executes_in_order() {
    let mut pipeline = ContextPipeline::new();
    pipeline.add(Box::new(AppendMiddleware { suffix: "A".into() }));
    pipeline.add(Box::new(AppendMiddleware { suffix: "B".into() }));
    let mut ctx = make_ctx();
    pipeline.execute(&mut ctx).await.unwrap();
    assert_eq!(ctx.system_prompt, "AB");
}

#[tokio::test]
async fn test_pipeline_stops_on_error() {
    let mut pipeline = ContextPipeline::new();
    pipeline.add(Box::new(FailMiddleware));
    pipeline.add(Box::new(AppendMiddleware { suffix: "X".into() }));
    let mut ctx = make_ctx();
    assert!(pipeline.execute(&mut ctx).await.is_err());
    assert_eq!(ctx.system_prompt, ""); // second middleware never ran
}

#[tokio::test]
async fn test_empty_pipeline() {
    let pipeline = ContextPipeline::new();
    let mut ctx = make_ctx();
    pipeline.execute(&mut ctx).await.unwrap();
}
