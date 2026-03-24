use loopal_context::compaction::{find_largest_result_block, sanitize_tool_pairs};
use loopal_context::{compact_messages, truncate_block_content};
use loopal_message::{ContentBlock, Message, MessageRole};

fn assistant_with_tool_use(id: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: id.to_string(),
            name: "read".to_string(),
            input: serde_json::json!({}),
        }],
    }
}

fn user_with_tool_result(tool_use_id: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: "ok".to_string(),
            is_error: false,
        }],
    }
}

#[test]
fn sanitize_removes_orphaned_tool_result() {
    let mut msgs = vec![
        Message::system("sys"),
        // tool_result without corresponding tool_use (tool_use was compacted away)
        user_with_tool_result("gone_id"),
        Message::assistant("done"),
    ];
    sanitize_tool_pairs(&mut msgs);
    // The orphaned tool_result message should be removed (empty after block removal)
    assert_eq!(msgs.len(), 2); // system + assistant
    assert_eq!(msgs[1].text_content(), "done");
}

#[test]
fn sanitize_removes_orphaned_tool_use() {
    let mut msgs = vec![
        Message::system("sys"),
        assistant_with_tool_use("orphan_id"),
        // No corresponding tool_result
        Message::user("next question"),
    ];
    sanitize_tool_pairs(&mut msgs);
    // tool_use block removed, but assistant message still has no content → removed
    assert_eq!(msgs.len(), 2); // system + user
}

#[test]
fn sanitize_preserves_valid_pairs() {
    let mut msgs = vec![
        Message::system("sys"),
        assistant_with_tool_use("valid_id"),
        user_with_tool_result("valid_id"),
        Message::assistant("response"),
    ];
    sanitize_tool_pairs(&mut msgs);
    assert_eq!(msgs.len(), 4); // all preserved
}

#[test]
fn compact_then_sanitize_fixes_broken_pairs() {
    let mut msgs = vec![
        Message::system("sys"),
        Message::user("q1"),
        assistant_with_tool_use("call_1"),
        user_with_tool_result("call_1"),
        Message::assistant("a1"),
        Message::user("q2"),
        assistant_with_tool_use("call_2"),
        user_with_tool_result("call_2"),
        Message::assistant("a2"),
    ];
    // compact keeps last 3 non-system → msgs 6,7,8 (tool_use "call_2", result, "a2")
    compact_messages(&mut msgs, 3);
    // After sanitize (called by compact_messages): all pairs should be valid
    for msg in &msgs {
        for block in &msg.content {
            if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                // Verify corresponding tool_use exists
                let has_use = msgs.iter().any(|m| {
                    m.content.iter().any(|b| {
                        matches!(
                            b, ContentBlock::ToolUse { id, .. } if id == tool_use_id
                        )
                    })
                });
                assert!(has_use, "orphaned tool_result: {tool_use_id}");
            }
        }
    }
}

// =============================================================================
// find_largest_result_block: ServerToolResult support
// =============================================================================

#[test]
fn find_largest_result_block_finds_server_tool_result() {
    let msgs = vec![
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ServerToolResult {
                block_type: "web_search_tool_result".to_string(),
                tool_use_id: "ws_1".to_string(),
                content: serde_json::json!({"data": "x".repeat(5000)}),
            }],
        },
        Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "t1".to_string(),
                content: "small".to_string(),
                is_error: false,
            }],
        },
    ];
    let (mi, bi, _size) = find_largest_result_block(&msgs).unwrap();
    assert_eq!(mi, 0); // ServerToolResult is larger
    assert_eq!(bi, 0);
}

#[test]
fn truncate_server_tool_result() {
    let big_json = serde_json::json!({"results": "x".repeat(5000)});
    let mut block = ContentBlock::ServerToolResult {
        block_type: "web_search_tool_result".to_string(),
        tool_use_id: "ws_1".to_string(),
        content: big_json,
    };
    truncate_block_content(&mut block, 20, 500);
    if let ContentBlock::ServerToolResult { content, .. } = &block {
        let owned = content.to_string();
        let s = content.as_str().unwrap_or(&owned);
        assert!(s.len() < 1000, "should be truncated: {}", s.len());
        assert!(s.contains("[Truncated:"));
    } else {
        panic!("expected ServerToolResult");
    }
}
