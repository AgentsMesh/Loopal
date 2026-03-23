use loopal_context::middleware::PriceLimit;
use loopal_message::Message;
use loopal_provider_api::{Middleware, MiddlewareContext};

/// Helper to build a MiddlewareContext with given messages and max_context_tokens.
fn make_ctx(messages: Vec<Message>, max_context_tokens: u32) -> MiddlewareContext {
    MiddlewareContext {
        messages,
        system_prompt: String::new(),
        model: "test-model".into(),
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cost: 0.0,
        max_context_tokens,
        compact_model: None,
        summarization_provider: None,
    }
}

// =============================================================================
// PriceLimit tests
// =============================================================================

#[tokio::test]
async fn price_limit_under_max_returns_ok() {
    let mw = PriceLimit::new(10.0);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.total_cost = 5.0;
    assert!(mw.process(&mut ctx).await.is_ok());
}

#[tokio::test]
async fn price_limit_at_max_returns_err() {
    let mw = PriceLimit::new(10.0);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.total_cost = 10.0;
    assert!(mw.process(&mut ctx).await.is_err());
}

#[tokio::test]
async fn price_limit_over_max_returns_err() {
    let mw = PriceLimit::new(5.0);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.total_cost = 7.5;
    assert!(mw.process(&mut ctx).await.is_err());
}

#[tokio::test]
async fn price_limit_zero_cost_ok() {
    let mw = PriceLimit::new(10.0);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.total_cost = 0.0;
    assert!(mw.process(&mut ctx).await.is_ok());
}

#[tokio::test]
async fn price_limit_error_message_contains_amounts() {
    let mw = PriceLimit::new(5.0);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.total_cost = 7.5;
    let err = mw.process(&mut ctx).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("7.5") && msg.contains("5.0"),
        "error message should contain cost ($7.5) and max ($5.0), got: {msg}"
    );
    assert!(
        msg.contains("price limit"),
        "error message should mention 'price limit', got: {msg}"
    );
}

#[tokio::test]
async fn price_limit_name() {
    let mw = PriceLimit::new(10.0);
    assert_eq!(mw.name(), "price_limit");
}
