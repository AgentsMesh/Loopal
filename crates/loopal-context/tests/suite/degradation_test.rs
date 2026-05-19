use loopal_context::budget::ContextBudget;
use loopal_context::degradation::run_sync_degradation;
use loopal_message::{ContentBlock, Message, MessageRole};

fn make_budget(message_budget: u32) -> ContextBudget {
    ContextBudget {
        context_window: message_budget * 2,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 0,
        safety_margin: 0,
        message_budget,
        max_output_tokens: 16_384,
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
        origin: None,
    }
}

fn tool_result_msg(content_size: usize) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: "x".repeat(content_size),
            images: Vec::new(),
            is_error: false,
            metadata: None,
        }],
        origin: None,
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

    assert!(
        messages[0]
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ServerToolResult { .. }))
    );
}

#[test]
fn layer1_truncates_oversized_old_tool_results_above_60_percent() {
    // Use random-looking text so cl100k_base can't compress it (repeated "x"
    // tokenizes very efficiently). budget = 10K → threshold per result =
    // 10K/8 = 1250 tokens. Each 50K-char unique body BPE's well above that
    // and total payload crosses 60% of the budget.
    let budget = make_budget(10_000);
    let bigfile_a: String = (0..50_000)
        .map(|i| (b'a' + (i % 26) as u8) as char)
        .collect();
    let bigfile_b: String = (0..50_000)
        .map(|i| (b'a' + ((i * 7) % 26) as u8) as char)
        .collect();
    let bigfile_c: String = (0..50_000)
        .map(|i| (b'a' + ((i * 13) % 26) as u8) as char)
        .collect();
    let mut messages = vec![
        Message::assistant("a"),
        msg_with_tool_result(&bigfile_a),
        Message::assistant("b"),
        msg_with_tool_result(&bigfile_b),
        Message::assistant("c"),
        msg_with_tool_result(&bigfile_c),
    ];

    run_sync_degradation(&mut messages, &budget);

    if let ContentBlock::ToolResult { content, .. } = &messages[1].content[0] {
        assert!(content.len() < 50_000, "old result should be truncated");
    }
    if let ContentBlock::ToolResult { content, .. } = &messages[5].content[0] {
        assert_eq!(content.len(), 50_000, "recent result preserved");
    }
}

fn msg_with_tool_result(content: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: content.to_string(),
            is_error: false,
            metadata: None,
        }],
        origin: None,
    }
}

#[test]
fn layer1_idle_under_60_percent() {
    let budget = make_budget(1_000_000);
    let mut messages = vec![
        Message::assistant("a"),
        tool_result_msg(50_000),
        Message::assistant("b"),
        tool_result_msg(50_000),
    ];

    run_sync_degradation(&mut messages, &budget);

    if let ContentBlock::ToolResult { content, .. } = &messages[1].content[0] {
        assert_eq!(
            content.len(),
            50_000,
            "should not truncate below 60% budget"
        );
    }
}
