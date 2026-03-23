use loopal_context::middleware::ContextGuard;
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

fn big_tool_result_message(size: usize) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: "x".repeat(size),
            is_error: false,
        }],
    }
}

fn large_message(n: usize) -> Message {
    Message::user(&"x".repeat(n))
}

#[tokio::test]
async fn truncates_large_tool_result_instead_of_dropping_messages() {
    // Scenario: only 3 messages, but one has a huge ToolResult.
    // Old behavior would skip compaction (3 < keep_last=10).
    // New behavior truncates the ToolResult.
    let mw = ContextGuard;
    let messages = vec![
        Message::user("hello"),
        big_tool_result_message(10_000), // ~2500 tokens
        Message::assistant("ok"),
    ];
    // max_context_tokens = 1000, threshold = 800 tokens.
    let mut ctx = make_ctx(messages, 1000);
    mw.process(&mut ctx).await.unwrap();

    // All 3 messages should still be present (not dropped)
    assert_eq!(ctx.messages.len(), 3);
    if let ContentBlock::ToolResult { content, .. } = &ctx.messages[1].content[0] {
        assert!(content.len() < 10_000, "ToolResult should be truncated");
        assert!(content.contains("[Truncated"));
    } else {
        panic!("expected ToolResult");
    }
}

#[tokio::test]
async fn falls_back_to_compact_when_no_large_tool_results() {
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
        "expected compaction fallback: {} vs {}",
        ctx.messages.len(),
        original_len
    );
}

#[tokio::test]
async fn iteratively_truncates_multiple_large_tool_results() {
    let mw = ContextGuard;
    let messages = vec![
        big_tool_result_message(8_000),
        big_tool_result_message(6_000),
        big_tool_result_message(4_000),
    ];
    // max_context_tokens = 500, threshold = 400.
    // Total ≈ (8000+6000+4000)/4 + 12 = 4512 >> 400
    let mut ctx = make_ctx(messages, 500);
    mw.process(&mut ctx).await.unwrap();

    // All messages should be present, but ToolResults truncated
    assert_eq!(ctx.messages.len(), 3);
    for msg in &ctx.messages {
        if let ContentBlock::ToolResult { content, .. } = &msg.content[0] {
            assert!(
                content.len() < 4_000,
                "ToolResult should be truncated, got {} bytes",
                content.len()
            );
        }
    }
}
