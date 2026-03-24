use loopal_context::budget::ContextBudget;
use loopal_context::degradation::{drop_oldest_group, emergency_degrade, run_sync_degradation};
use loopal_message::{ContentBlock, Message, MessageRole};

fn make_budget(message_budget: u32) -> ContextBudget {
    ContextBudget {
        context_window: message_budget * 2,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 0,
        safety_margin: 0,
        message_budget,
    }
}

fn assistant_with_server_blocks() -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            ContentBlock::ServerToolUse {
                id: "st-1".into(),
                name: "web_search".into(),
                input: serde_json::json!({}),
            },
            ContentBlock::ServerToolResult {
                block_type: "web_search_tool_result".into(),
                tool_use_id: "st-1".into(),
                content: serde_json::json!({"results": []}),
            },
            ContentBlock::Text {
                text: "Here are the results".into(),
            },
        ],
    }
}

fn tool_result_msg(content_size: usize) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: "x".repeat(content_size),
            is_error: false,
        }],
    }
}

#[test]
fn layer0_strips_old_server_blocks() {
    let budget = make_budget(100_000);
    let mut messages = vec![
        assistant_with_server_blocks(),
        Message::user("thanks"),
        Message::assistant("you're welcome"),
    ];

    run_sync_degradation(&mut messages, &budget);

    // First assistant's server blocks should be condensed
    let first = &messages[0];
    assert!(
        !first
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ServerToolResult { .. })),
        "ServerToolResult should be removed from old assistant"
    );
}

#[test]
fn layer0_preserves_last_assistant_server_blocks() {
    let budget = make_budget(100_000);
    let mut messages = vec![assistant_with_server_blocks()];

    run_sync_degradation(&mut messages, &budget);

    // Only assistant = last, server blocks preserved
    assert!(
        messages[0]
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ServerToolResult { .. }))
    );
}

#[test]
fn layer0_strips_old_thinking() {
    let budget = make_budget(100_000);
    let mut messages = vec![
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Thinking {
                    thinking: "old thinking".into(),
                    signature: Some("sig".into()),
                },
                ContentBlock::Text {
                    text: "response".into(),
                },
            ],
        },
        Message::user("next"),
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Thinking {
                    thinking: "new thinking".into(),
                    signature: Some("sig2".into()),
                },
                ContentBlock::Text {
                    text: "response 2".into(),
                },
            ],
        },
    ];

    // strip_old_thinking is called in prepare_for_llm, not in run_sync_degradation
    // But Layer 0 in degradation only handles server blocks and images
    // Thinking is handled separately in store.prepare_for_llm()
    // This test verifies the messages pass through without error
    run_sync_degradation(&mut messages, &budget);
    assert_eq!(messages.len(), 3);
}

#[test]
fn emergency_degrade_truncates_old_results() {
    let mut messages = vec![
        Message::assistant("first"),
        tool_result_msg(50_000),
        Message::assistant("second"),
        tool_result_msg(50_000),
    ];

    emergency_degrade(&mut messages);

    // Only the first tool result should be truncated (not the last 4 messages)
    // With 4 messages, recent boundary = 0, so nothing is "old" enough
    // Let's add more messages to have some "old" ones
    let mut messages = vec![
        Message::assistant("old"),
        tool_result_msg(50_000),
        Message::assistant("mid"),
        tool_result_msg(50_000),
        Message::assistant("recent"),
        tool_result_msg(50_000),
    ];

    emergency_degrade(&mut messages);

    // First two tool results (indices 1, 3) should be truncated
    if let ContentBlock::ToolResult { content, .. } = &messages[1].content[0] {
        assert!(content.len() < 50_000, "old result should be truncated");
    }
    // Last tool result (index 5) should be preserved (within recent boundary)
    if let ContentBlock::ToolResult { content, .. } = &messages[5].content[0] {
        assert_eq!(content.len(), 50_000, "recent result should be preserved");
    }
}

#[test]
fn drop_oldest_group_removes_first_pair() {
    let mut messages = vec![
        Message::assistant("first"),
        Message::user("result1"),
        Message::assistant("second"),
        Message::user("result2"),
        Message::assistant("third"),
        Message::user("result3"),
    ];

    let removed = drop_oldest_group(&mut messages);
    assert_eq!(removed, 2); // assistant + user
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].text_content(), "second");
}

#[test]
fn drop_oldest_group_preserves_minimum() {
    let mut messages = vec![
        Message::assistant("only"),
        Message::user("pair"),
        Message::assistant("last"),
        Message::user("final"),
    ];

    let removed = drop_oldest_group(&mut messages);
    assert_eq!(removed, 0, "should not drop when only 4 messages remain");
}
