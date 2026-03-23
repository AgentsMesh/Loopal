use loopal_context::middleware::ContextGuard;
use loopal_context::token_counter::estimate_message_tokens;
use loopal_message::Message;
use loopal_provider_api::{Middleware, MiddlewareContext};

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

#[tokio::test]
async fn context_guard_under_threshold_no_compaction() {
    let mw = ContextGuard;
    let messages = vec![
        Message::user("hello"),
        Message::assistant("hi"),
        Message::user("bye"),
    ];
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 100_000);
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), original_len);
}

#[tokio::test]
async fn context_guard_over_threshold_triggers_compaction() {
    let mw = ContextGuard;
    let mut messages = vec![Message::system("sys")];
    for _ in 0..20 {
        messages.push(Message::user(&"x".repeat(400)));
    }
    let original_len = messages.len();
    // Low limit ensures total tokens far exceed 80% threshold
    let mut ctx = make_ctx(messages, 100);
    mw.process(&mut ctx).await.unwrap();
    assert!(
        ctx.messages.len() < original_len,
        "expected compaction from {original_len} but got {}",
        ctx.messages.len()
    );
}

#[tokio::test]
async fn context_guard_at_threshold_no_compaction() {
    let mw = ContextGuard;
    let msg = Message::user("hello world");
    let msg_tokens = estimate_message_tokens(&msg);
    // Set max so that msg_tokens is exactly at 80% threshold
    let max_ctx = (msg_tokens as f64 / 0.8).ceil() as u32;
    let mut ctx = make_ctx(vec![msg], max_ctx);
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), 1);
}

#[tokio::test]
async fn context_guard_just_above_threshold_triggers_compaction() {
    let mw = ContextGuard;
    // 20 messages each with enough text to exceed 80% of a small window
    let mut messages = vec![Message::system("s")];
    for _ in 0..20 {
        messages.push(Message::user(&"b".repeat(200)));
    }
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 1000);
    mw.process(&mut ctx).await.unwrap();
    assert!(
        ctx.messages.len() < original_len,
        "expected compaction above threshold, from {original_len} to {}",
        ctx.messages.len()
    );
}

#[tokio::test]
async fn context_guard_name() {
    let mw = ContextGuard;
    assert_eq!(mw.name(), "context_guard");
}
