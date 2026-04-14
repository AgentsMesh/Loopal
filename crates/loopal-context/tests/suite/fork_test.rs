use loopal_context::fork::{FORK_BOILERPLATE, compress_for_fork};
use loopal_message::{ContentBlock, Message, MessageRole};

#[test]
fn empty_input() {
    assert!(compress_for_fork(&[]).is_empty());
}

#[test]
fn strips_thinking_blocks() {
    let msgs = vec![Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            ContentBlock::Thinking {
                thinking: "hmm".into(),
                signature: None,
            },
            ContentBlock::Text {
                text: "hello".into(),
            },
        ],
    }];
    let result = compress_for_fork(&msgs);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content.len(), 1);
    assert!(matches!(&result[0].content[0], ContentBlock::Text { .. }));
}

#[test]
fn strips_incomplete_tail() {
    let msgs = vec![
        Message::user("q"),
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "t1".into(),
                name: "Agent".into(),
                input: serde_json::json!({}),
            }],
        },
    ];
    let result = compress_for_fork(&msgs);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].role, MessageRole::User);
}

#[test]
fn truncates_long_tool_result() {
    let long_content = "x".repeat(500);
    let msgs = vec![Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: long_content,
            is_error: false,
            metadata: None,
        }],
    }];
    let result = compress_for_fork(&msgs);
    let ContentBlock::ToolResult { content, .. } = &result[0].content[0] else {
        panic!("expected ToolResult");
    };
    assert!(content.len() < 300);
    assert!(content.ends_with("…[truncated]"));
}

#[test]
fn boilerplate_contains_no_spawn_rule() {
    assert!(FORK_BOILERPLATE.contains("Do NOT spawn sub-agents"));
}

#[test]
fn utf8_truncation_no_panic() {
    let content = "a".repeat(100) + &"😀".repeat(50);
    let msgs = vec![Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content,
            is_error: false,
            metadata: None,
        }],
    }];
    let result = compress_for_fork(&msgs);
    assert!(!result.is_empty());
}

#[test]
fn result_starts_with_user_message() {
    let msgs = vec![
        Message::user("q1"),
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::Text { text: "a1".into() }],
        },
        Message::user("q2"),
    ];
    let result = compress_for_fork(&msgs);
    assert!(!result.is_empty());
    assert_eq!(result[0].role, MessageRole::User);
}

#[test]
fn fork_context_json_round_trip() {
    let msgs = vec![
        Message::user("what files exist?"),
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Text { text: "Let me check.".into() },
                ContentBlock::ToolUse {
                    id: "tu1".into(),
                    name: "Glob".into(),
                    input: serde_json::json!({"pattern": "**/*.rs"}),
                },
            ],
        },
        Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "tu1".into(),
                content: "src/main.rs\nsrc/lib.rs".into(),
                is_error: false,
                metadata: None,
            }],
        },
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::Text {
                text: "Found 2 files.".into(),
            }],
        },
    ];
    let compressed = compress_for_fork(&msgs);
    assert!(!compressed.is_empty());

    let json_val = serde_json::to_value(&compressed).expect("serialize");
    let recovered: Vec<Message> =
        serde_json::from_value(json_val).expect("deserialize");

    assert_eq!(recovered.len(), compressed.len());
    for (orig, recov) in compressed.iter().zip(recovered.iter()) {
        assert_eq!(orig.role, recov.role);
        assert_eq!(orig.content.len(), recov.content.len());
    }
}