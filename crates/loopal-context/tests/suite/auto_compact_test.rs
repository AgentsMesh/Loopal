use loopal_context::middleware::{AutoCompact, SmartCompact};
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

/// Generate a large message with `n` chars of text content.
fn large_message(n: usize) -> Message {
    Message::user(&"x".repeat(n))
}

// =============================================================================
// AutoCompact tests
// =============================================================================

#[tokio::test]
async fn auto_compact_under_limit_no_change() {
    let mw = AutoCompact::new(5);
    let messages = vec![Message::user("hello"), Message::assistant("hi")];
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 100_000);
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), original_len);
}

#[tokio::test]
async fn auto_compact_over_limit_triggers_compaction() {
    let mw = AutoCompact::new(2);
    // max_context_tokens = 50, each large_message(400) ~ 104 tokens
    let mut messages = vec![Message::system("sys")];
    for _ in 0..15 {
        messages.push(large_message(400));
    }
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 50);
    mw.process(&mut ctx).await.unwrap();
    assert!(
        ctx.messages.len() < original_len,
        "expected compaction to reduce messages from {} but got {}",
        original_len,
        ctx.messages.len()
    );
}

#[tokio::test]
async fn auto_compact_keeps_specified_last_count() {
    let keep_last = 3;
    let mw = AutoCompact::new(keep_last);
    let mut messages = vec![Message::system("sys")];
    for _ in 0..10 {
        messages.push(large_message(400));
    }
    let mut ctx = make_ctx(messages, 50);
    mw.process(&mut ctx).await.unwrap();
    // After compaction: system + keep_last non-system messages
    assert_eq!(ctx.messages.len(), 1 + keep_last);
}

#[tokio::test]
async fn auto_compact_preserves_last_messages() {
    let keep_last = 2;
    let mw = AutoCompact::new(keep_last);
    let mut messages = vec![Message::system("system prompt")];
    messages.push(Message::user("first user msg"));
    messages.push(Message::assistant("first assistant msg"));
    messages.push(Message::user("second user msg")); // kept
    messages.push(Message::assistant("second assistant msg")); // kept
    let mut ctx = make_ctx(messages, 1); // very low limit
    mw.process(&mut ctx).await.unwrap();

    assert_eq!(ctx.messages.len(), 1 + keep_last);
    assert_eq!(ctx.messages[0].text_content(), "system prompt");
    assert_eq!(ctx.messages[1].text_content(), "second user msg");
    assert_eq!(ctx.messages[2].text_content(), "second assistant msg");
}

#[tokio::test]
async fn auto_compact_name() {
    let mw = AutoCompact::new(5);
    assert_eq!(mw.name(), "auto_compact");
}

// =============================================================================
// SmartCompact tests
// =============================================================================

#[tokio::test]
async fn smart_compact_under_limit_no_change() {
    let mw = SmartCompact::new(5);
    let messages = vec![Message::user("hello"), Message::assistant("hi")];
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 100_000);
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), original_len);
}

#[tokio::test]
async fn smart_compact_over_limit_no_provider_falls_back_to_truncation() {
    let mw = SmartCompact::new(2);
    let mut messages = vec![Message::system("sys")];
    for _ in 0..15 {
        messages.push(large_message(400));
    }
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 50);
    mw.process(&mut ctx).await.unwrap();
    assert!(
        ctx.messages.len() < original_len,
        "expected truncation fallback to reduce messages from {} but got {}",
        original_len,
        ctx.messages.len()
    );
}
