use loopal_context::middleware::{ContextGuard, TurnLimit};
use loopal_types::message::Message;
use loopal_types::middleware::{Middleware, MiddlewareContext};

/// Helper to build a MiddlewareContext with given messages and max_context_tokens.
fn make_ctx(messages: Vec<Message>, max_context_tokens: u32) -> MiddlewareContext {
    MiddlewareContext {
        messages,
        system_prompt: String::new(),
        model: "test-model".into(),
        turn_count: 0,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cost: 0.0,
        max_context_tokens,
        summarization_provider: None,
    }
}

/// Generate a large message with `n` chars of text content.
fn large_message(n: usize) -> Message {
    Message::user(&"x".repeat(n))
}

// =============================================================================
// TurnLimit tests
// =============================================================================

#[tokio::test]
async fn turn_limit_under_max_returns_ok() {
    let mw = TurnLimit::new(10);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.turn_count = 5;
    assert!(mw.process(&mut ctx).await.is_ok());
}

#[tokio::test]
async fn turn_limit_at_max_returns_err() {
    let mw = TurnLimit::new(10);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.turn_count = 10;
    assert!(mw.process(&mut ctx).await.is_err());
}

#[tokio::test]
async fn turn_limit_over_max_returns_err() {
    let mw = TurnLimit::new(5);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.turn_count = 20;
    assert!(mw.process(&mut ctx).await.is_err());
}

#[tokio::test]
async fn turn_limit_zero_turns_ok() {
    let mw = TurnLimit::new(10);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.turn_count = 0;
    assert!(mw.process(&mut ctx).await.is_ok());
}

#[tokio::test]
async fn turn_limit_error_message_contains_counts() {
    let mw = TurnLimit::new(5);
    let mut ctx = make_ctx(vec![], 100_000);
    ctx.turn_count = 7;
    let err = mw.process(&mut ctx).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("7") && msg.contains("5"),
        "error message should contain turn count (7) and max (5), got: {}",
        msg
    );
    assert!(
        msg.contains("turn limit"),
        "error message should mention 'turn limit', got: {}",
        msg
    );
}

#[tokio::test]
async fn turn_limit_name() {
    let mw = TurnLimit::new(10);
    assert_eq!(mw.name(), "turn_limit");
}

// =============================================================================
// ContextGuard tests
// =============================================================================

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
        messages.push(large_message(400));
    }
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 100);
    mw.process(&mut ctx).await.unwrap();
    assert!(
        ctx.messages.len() < original_len,
        "expected compaction to reduce messages from {} but got {}",
        original_len,
        ctx.messages.len()
    );
}

#[tokio::test]
async fn context_guard_exactly_at_threshold_no_compaction() {
    let mw = ContextGuard;
    // threshold = max_context_tokens * 0.8 = 1000 * 0.8 = 800
    // N/4 + 4 = 800 => N = 3184
    let messages = vec![Message::user(&"a".repeat(3184))];
    let mut ctx = make_ctx(messages, 1000);
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), 1);
}

#[tokio::test]
async fn context_guard_just_below_threshold_no_compaction() {
    let mw = ContextGuard;
    // threshold = 800. N/4 + 4 = 799 => N = 3180
    let messages = vec![Message::user(&"a".repeat(3180))];
    let mut ctx = make_ctx(messages, 1000);
    mw.process(&mut ctx).await.unwrap();
    assert_eq!(ctx.messages.len(), 1);
}

#[tokio::test]
async fn context_guard_just_above_threshold_triggers_compaction() {
    let mw = ContextGuard;
    // 20 messages, each with text of length 200 => each ~54 tokens => total ~1080 > 800
    let mut messages = vec![Message::system("s")];
    for _ in 0..20 {
        messages.push(Message::user(&"b".repeat(200)));
    }
    let original_len = messages.len();
    let mut ctx = make_ctx(messages, 1000);
    mw.process(&mut ctx).await.unwrap();
    assert!(
        ctx.messages.len() < original_len,
        "expected compaction above threshold, from {} to {}",
        original_len,
        ctx.messages.len()
    );
}

#[tokio::test]
async fn context_guard_name() {
    let mw = ContextGuard;
    assert_eq!(mw.name(), "context_guard");
}
