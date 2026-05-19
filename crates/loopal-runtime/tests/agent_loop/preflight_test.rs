use loopal_context::ingestion::safe_truncate_tool_result;
use loopal_context::token_counter::estimate_messages_tokens;
use loopal_message::{ContentBlock, Message, MessageRole};

fn big_tool_result_message(size: usize) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: "x".repeat(size),
            images: Vec::new(),
            is_error: false,
            metadata: None,
        }],
        origin: None,
    }
}

#[test]
fn safe_truncation_reduces_token_estimate() {
    let mut messages = vec![
        Message::user("hello"),
        big_tool_result_message(40_000),
        Message::assistant("ok"),
    ];

    let before = estimate_messages_tokens(&messages);
    assert!(before > 5000, "should be large: {before}");

    safe_truncate_tool_result(&mut messages[1].content[0], 20, 500);
    let after = estimate_messages_tokens(&messages);

    assert!(
        after < before / 2,
        "should be significantly reduced: before={before}, after={after}"
    );
}

#[test]
fn safe_truncation_skips_server_tool_result() {
    let mut block = ContentBlock::ServerToolResult {
        block_type: "web_search_tool_result".into(),
        tool_use_id: "ws_1".into(),
        content: serde_json::json!({"results": "x".repeat(5000)}),
    };

    safe_truncate_tool_result(&mut block, 20, 500);

    // Must still be ServerToolResult with valid JSON — never corrupted
    assert!(matches!(block, ContentBlock::ServerToolResult { .. }));
    if let ContentBlock::ServerToolResult { content, .. } = &block {
        assert!(content.is_object(), "JSON structure must be preserved");
    }
}

#[test]
fn sanitize_no_tool_results_simple_drain() {
    use loopal_context::sanitize_tool_pairs;

    let mut messages: Vec<Message> = (0..20).map(|_| Message::user(&"y".repeat(1000))).collect();
    messages.drain(0..17);
    sanitize_tool_pairs(&mut messages);
    assert_eq!(messages.len(), 3);
}

#[test]
fn iterative_safe_truncation_handles_multiple_blocks() {
    let mut messages = vec![
        big_tool_result_message(20_000),
        big_tool_result_message(15_000),
        big_tool_result_message(10_000),
    ];

    for msg in &mut messages {
        for block in &mut msg.content {
            safe_truncate_tool_result(block, 20, 500);
        }
    }

    assert_eq!(messages.len(), 3);
    for msg in &messages {
        if let ContentBlock::ToolResult { content, .. } = &msg.content[0] {
            assert!(
                content.len() < 5000,
                "should be truncated: {}",
                content.len()
            );
        }
    }
}
