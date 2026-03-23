use loopal_context::middleware::MessageSizeGuard;
use loopal_message::{ContentBlock, Message, MessageRole};
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

fn tool_result_message(content: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: content.to_string(),
            is_error: false,
        }],
    }
}

#[tokio::test]
async fn small_message_untouched() {
    let mw = MessageSizeGuard;
    let msg = tool_result_message("small result");
    let mut ctx = make_ctx(vec![msg], 100_000);

    mw.process(&mut ctx).await.unwrap();

    // Message should not be modified
    assert_eq!(ctx.messages.len(), 1);
    if let ContentBlock::ToolResult { content, .. } = &ctx.messages[0].content[0] {
        assert_eq!(content, "small result");
    }
}

#[tokio::test]
async fn oversized_tool_result_gets_truncated() {
    let mw = MessageSizeGuard;
    // max_context_tokens = 1000, threshold = 250 tokens.
    // A message with 8000 chars ≈ 2000 tokens > 250 threshold.
    // TRUNCATED_MAX_BYTES = 2000, so 8000 > 2000 triggers truncation.
    let big = "x".repeat(8000);
    let msg = tool_result_message(&big);
    let mut ctx = make_ctx(vec![msg], 1000);

    mw.process(&mut ctx).await.unwrap();

    if let ContentBlock::ToolResult { content, .. } = &ctx.messages[0].content[0] {
        assert!(content.len() < 8000, "should be truncated");
        assert!(content.contains("[Truncated by context guard"));
    } else {
        panic!("expected ToolResult");
    }
}

#[tokio::test]
async fn text_only_message_not_truncated() {
    let mw = MessageSizeGuard;
    // Large text message — no ToolResult to truncate, so nothing changes.
    let big_text = "y".repeat(5000);
    let msg = Message::user(&big_text);
    let mut ctx = make_ctx(vec![msg], 1000);

    mw.process(&mut ctx).await.unwrap();

    // Text message should pass through (no ToolResult to truncate)
    assert_eq!(ctx.messages[0].text_content().len(), 5000);
}

#[tokio::test]
async fn multiple_messages_only_oversized_truncated() {
    let mw = MessageSizeGuard;
    let small = tool_result_message("ok");
    let big = tool_result_message(&"z".repeat(8000));
    let mut ctx = make_ctx(vec![small, big], 1000);

    mw.process(&mut ctx).await.unwrap();

    // First message untouched
    if let ContentBlock::ToolResult { content, .. } = &ctx.messages[0].content[0] {
        assert_eq!(content, "ok");
    }
    // Second message truncated
    if let ContentBlock::ToolResult { content, .. } = &ctx.messages[1].content[0] {
        assert!(content.len() < 8000);
        assert!(content.contains("[Truncated"));
    }
}

#[tokio::test]
async fn name_is_correct() {
    let mw = MessageSizeGuard;
    assert_eq!(mw.name(), "message_size_guard");
}
