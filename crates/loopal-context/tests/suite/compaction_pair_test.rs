use loopal_context::compact_messages;
use loopal_context::compaction::sanitize_tool_pairs;
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
        user_with_tool_result("gone_id"),
        Message::assistant("done"),
    ];
    sanitize_tool_pairs(&mut msgs);
    assert_eq!(msgs.len(), 2); // system + assistant
    assert_eq!(msgs[1].text_content(), "done");
}

#[test]
fn sanitize_removes_orphaned_tool_use() {
    let mut msgs = vec![
        Message::system("sys"),
        assistant_with_tool_use("orphan_id"),
        Message::user("next question"),
    ];
    sanitize_tool_pairs(&mut msgs);
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
    assert_eq!(msgs.len(), 4);
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
    compact_messages(&mut msgs, 3);
    for msg in &msgs {
        for block in &msg.content {
            if let ContentBlock::ToolResult { tool_use_id, .. } = block {
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
