use loopal_context::ingestion::{
    cap_tool_results, condense_old_server_blocks, safe_truncate_tool_result,
};
use loopal_message::{ContentBlock, Message, MessageRole};

/// Helper: create a ToolResult block with given content size.
fn tool_result(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: content.to_string(),
        is_error: false,
    }
}

fn server_tool_use(name: &str) -> ContentBlock {
    ContentBlock::ServerToolUse {
        id: "st-1".to_string(),
        name: name.to_string(),
        input: serde_json::json!({}),
    }
}

fn server_tool_result(block_type: &str) -> ContentBlock {
    ContentBlock::ServerToolResult {
        block_type: block_type.to_string(),
        tool_use_id: "st-1".to_string(),
        content: serde_json::json!({"results": [{"title": "test", "url": "http://example.com"}]}),
    }
}

#[test]
fn cap_tool_results_truncates_oversized() {
    let big_content = "x".repeat(100_000);
    let mut msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![tool_result("t1", &big_content)],
    };
    // max_tokens = 1000, the big content is ~25000 tokens
    cap_tool_results(&mut msg, 1_000);

    if let ContentBlock::ToolResult { content, .. } = &msg.content[0] {
        assert!(content.len() < 100_000, "should be truncated");
        assert!(
            content.contains("Truncated"),
            "should have truncation notice"
        );
    } else {
        panic!("expected ToolResult");
    }
}

#[test]
fn cap_tool_results_preserves_small() {
    let small_content = "hello world";
    let mut msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![tool_result("t1", small_content)],
    };
    cap_tool_results(&mut msg, 1_000);

    if let ContentBlock::ToolResult { content, .. } = &msg.content[0] {
        assert_eq!(content, small_content);
    }
}

#[test]
fn cap_tool_results_skips_errors() {
    let big_content = "x".repeat(100_000);
    let mut msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: big_content.clone(),
            is_error: true,
        }],
    };
    cap_tool_results(&mut msg, 1_000);
    if let ContentBlock::ToolResult { content, .. } = &msg.content[0] {
        assert_eq!(
            content.len(),
            big_content.len(),
            "errors should not be truncated"
        );
    }
}

#[test]
fn condense_old_server_blocks_strips_non_last() {
    let mut messages = vec![
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                server_tool_use("web_search"),
                server_tool_result("web_search_tool_result"),
                ContentBlock::Text {
                    text: "Found results".into(),
                },
            ],
        },
        Message::user("thanks"),
        Message::assistant("you're welcome"),
    ];

    condense_old_server_blocks(&mut messages);

    // First assistant: server blocks should be condensed
    assert_eq!(messages[0].content.len(), 2); // Text replacement + original Text
    if let ContentBlock::Text { text } = &messages[0].content[0] {
        assert!(text.contains("web_search"), "should mention tool name");
    }
    // Last assistant: unchanged
    assert_eq!(messages[2].content.len(), 1);
}

#[test]
fn condense_preserves_last_assistant_server_blocks() {
    let mut messages = vec![Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            server_tool_use("web_search"),
            server_tool_result("web_search_tool_result"),
            ContentBlock::Text {
                text: "Results".into(),
            },
        ],
    }];

    condense_old_server_blocks(&mut messages);

    // Only one assistant = last, should be preserved
    assert_eq!(messages[0].content.len(), 3);
}

#[test]
fn safe_truncate_skips_server_tool_result() {
    let mut block = server_tool_result("web_search_tool_result");
    safe_truncate_tool_result(&mut block, 10, 100);

    // Must still be ServerToolResult — never converted to string
    assert!(matches!(block, ContentBlock::ServerToolResult { .. }));
}

#[test]
fn safe_truncate_works_on_tool_result() {
    let big = "line\n".repeat(1000);
    let mut block = tool_result("t1", &big);
    safe_truncate_tool_result(&mut block, 10, 200);

    if let ContentBlock::ToolResult { content, .. } = &block {
        assert!(content.len() < big.len());
        assert!(content.contains("Truncated"));
    }
}
