use loopal_message::{ContentBlock, Message, MessageRole};

/// Verify the preflight helper functions from the compaction module
/// (which preflight.rs delegates to) work correctly in the context
/// they'd be used during a preflight check.
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

#[test]
fn preflight_truncation_reduces_token_estimate() {
    use loopal_context::compaction::{find_largest_result_block, truncate_block_content};
    use loopal_context::token_counter::estimate_messages_tokens;

    let mut messages = vec![
        Message::user("hello"),
        big_tool_result_message(40_000), // ~10k tokens
        Message::assistant("ok"),
    ];

    let before = estimate_messages_tokens(&messages);
    assert!(before > 5000, "should be large: {before}");

    let (mi, bi, size) = find_largest_result_block(&messages).unwrap();
    assert_eq!(mi, 1);
    assert_eq!(size, 40_000);

    truncate_block_content(&mut messages[mi].content[bi], 20, 500);
    let after = estimate_messages_tokens(&messages);

    assert!(
        after < before / 2,
        "should be significantly reduced: before={before}, after={after}"
    );
}

#[test]
fn preflight_compact_fallback_with_no_tool_results() {
    use loopal_context::compact_messages;
    use loopal_context::compaction::find_largest_result_block;

    let mut messages: Vec<Message> = (0..20).map(|_| Message::user(&"y".repeat(1000))).collect();

    assert!(find_largest_result_block(&messages).is_none());

    compact_messages(&mut messages, 3);
    assert_eq!(messages.len(), 3);
}

#[test]
fn preflight_iterative_truncation_handles_multiple_blocks() {
    use loopal_context::compaction::{find_largest_result_block, truncate_block_content};
    use loopal_context::token_counter::estimate_messages_tokens;

    let mut messages = vec![
        big_tool_result_message(20_000),
        big_tool_result_message(15_000),
        big_tool_result_message(10_000),
    ];

    // Simulate iterative truncation like preflight does
    for _ in 0..5 {
        let tokens = estimate_messages_tokens(&messages);
        if tokens < 500 {
            break;
        }
        if let Some((mi, bi, size)) = find_largest_result_block(&messages) {
            if size < 1000 {
                break;
            }
            truncate_block_content(&mut messages[mi].content[bi], 20, 500);
        }
    }

    // All messages should still exist
    assert_eq!(messages.len(), 3);
    // All ToolResults should be truncated
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
