use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::projection::project_messages;

fn text_msg(role: MessageRole, text: &str) -> Message {
    Message {
        id: None,
        role,
        content: vec![ContentBlock::Text {
            text: text.to_string(),
        }],
    }
}

#[test]
fn project_empty() {
    let result = project_messages(&[]);
    assert!(result.is_empty());
}

#[test]
fn project_plain_text() {
    let msgs = vec![
        text_msg(MessageRole::User, "hello"),
        text_msg(MessageRole::Assistant, "hi"),
    ];
    let display = project_messages(&msgs);
    assert_eq!(display.len(), 2);
    assert_eq!(display[0].role, "user");
    assert_eq!(display[0].content, "hello");
    assert_eq!(display[0].image_count, 0);
    assert_eq!(display[1].role, "assistant");
    assert_eq!(display[1].content, "hi");
    assert_eq!(display[1].image_count, 0);
}

#[test]
fn project_tool_use_and_result() {
    let assistant_msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            ContentBlock::Text {
                text: "Let me read that.".into(),
            },
            ContentBlock::ToolUse {
                id: "tu-1".into(),
                name: "Read".into(),
                input: serde_json::json!({"path": "/tmp/foo"}),
            },
        ],
    };
    let user_msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tu-1".into(),
            content: "file contents here".into(),
            is_error: false,
            is_completion: false,
            metadata: None,
        }],
    };
    let display = project_messages(&[assistant_msg, user_msg]);
    assert_eq!(display.len(), 1);
    assert_eq!(display[0].content, "Let me read that.");
    assert_eq!(display[0].tool_calls.len(), 1);
    assert_eq!(display[0].tool_calls[0].name, "Read");
    assert!(!display[0].tool_calls[0].is_error);
    assert!(display[0].tool_calls[0].result.is_some());
}

#[test]
fn project_tool_use_error() {
    let assistant_msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "tu-err".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "exit 1"}),
        }],
    };
    let user_msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tu-err".into(),
            content: "command failed".into(),
            is_error: true,
            is_completion: false,
            metadata: None,
        }],
    };
    let display = project_messages(&[assistant_msg, user_msg]);
    assert!(display[0].tool_calls[0].is_error);
}

#[test]
fn project_image_placeholder() {
    let msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Image {
            source: loopal_message::ImageSource {
                source_type: "base64".into(),
                media_type: "image/png".into(),
                data: "iVBOR...".into(),
            },
        }],
    };
    let display = project_messages(&[msg]);
    assert_eq!(display[0].content, "[image]");
    assert_eq!(display[0].image_count, 1);
}

#[test]
fn project_multi_turn_mixed() {
    let msgs = vec![
        text_msg(MessageRole::User, "q1"),
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "doing".into(),
                },
                ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Glob".into(),
                    input: serde_json::json!({"pattern": "*.rs"}),
                },
            ],
        },
        Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "t1".into(),
                content: "main.rs".into(),
                is_error: false,
                is_completion: false,
                metadata: None,
            }],
        },
        text_msg(MessageRole::Assistant, "done"),
        text_msg(MessageRole::User, "q2"),
    ];
    let display = project_messages(&msgs);
    assert_eq!(display.len(), 4);
    assert_eq!(display[0].role, "user");
    assert_eq!(display[1].tool_calls.len(), 1);
    assert!(!display[1].tool_calls[0].is_error);
    assert_eq!(display[2].content, "done");
    assert_eq!(display[3].content, "q2");
}

#[test]
fn project_skips_empty_messages() {
    let msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![],
    };
    let display = project_messages(&[msg]);
    assert!(display.is_empty());
}
