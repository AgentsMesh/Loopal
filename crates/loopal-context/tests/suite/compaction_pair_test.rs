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
            is_completion: false,
            metadata: None,
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

fn assistant_with_server_tool_blocks(blocks: Vec<ContentBlock>) -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content: blocks,
    }
}

fn server_tool_use(id: &str) -> ContentBlock {
    server_tool_use_with_input(id, serde_json::json!({"code": "test()"}))
}

fn server_tool_use_with_input(id: &str, input: serde_json::Value) -> ContentBlock {
    ContentBlock::ServerToolUse {
        id: id.to_string(),
        name: "code_execution".to_string(),
        input,
    }
}

fn server_tool_result(tool_use_id: &str) -> ContentBlock {
    ContentBlock::ServerToolResult {
        block_type: "code_execution_tool_result".to_string(),
        tool_use_id: tool_use_id.to_string(),
        content: serde_json::json!({"output": "ok"}),
    }
}

#[test]
fn sanitize_removes_orphaned_server_tool_use() {
    let mut msgs = vec![
        Message::system("sys"),
        assistant_with_server_tool_blocks(vec![
            server_tool_use("srv_1"),
            server_tool_result("srv_1"),
            server_tool_use("srv_orphan"), // no matching result
        ]),
        Message::user("continue"),
    ];
    sanitize_tool_pairs(&mut msgs);
    assert_eq!(msgs.len(), 3);
    // orphaned ServerToolUse removed, matched pair preserved
    assert_eq!(msgs[1].content.len(), 2);
}

#[test]
fn sanitize_removes_orphaned_server_tool_result() {
    let mut msgs = vec![
        Message::system("sys"),
        assistant_with_server_tool_blocks(vec![
            server_tool_result("srv_gone"), // no matching use
        ]),
        Message::user("next"),
    ];
    sanitize_tool_pairs(&mut msgs);
    // assistant message becomes empty → removed
    assert_eq!(msgs.len(), 2); // system + user
}

#[test]
fn sanitize_preserves_valid_server_tool_pairs() {
    let mut msgs = vec![
        Message::system("sys"),
        assistant_with_server_tool_blocks(vec![
            server_tool_use("srv_a"),
            server_tool_use("srv_b"),
            server_tool_result("srv_b"),
            server_tool_result("srv_a"),
        ]),
        Message::user("done"),
    ];
    sanitize_tool_pairs(&mut msgs);
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[1].content.len(), 4); // all preserved
}

#[test]
fn sanitize_strips_code_execution_with_empty_input() {
    let mut msgs = vec![
        Message::system("sys"),
        assistant_with_server_tool_blocks(vec![
            server_tool_use_with_input("ce_empty", serde_json::json!({})), // empty → stripped
            server_tool_result("ce_empty"),
        ]),
        Message::user("continue"),
    ];
    sanitize_tool_pairs(&mut msgs);
    // Both blocks removed: empty-input use stripped, orphaned result follows
    assert_eq!(msgs.len(), 2); // system + user
}

#[test]
fn sanitize_preserves_code_execution_with_valid_input() {
    let mut msgs = vec![
        Message::system("sys"),
        assistant_with_server_tool_blocks(vec![
            server_tool_use_with_input("ce_valid", serde_json::json!({"code": "print(1)"})),
            server_tool_result("ce_valid"),
        ]),
        Message::user("continue"),
    ];
    sanitize_tool_pairs(&mut msgs);
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[1].content.len(), 2); // pair preserved
}
