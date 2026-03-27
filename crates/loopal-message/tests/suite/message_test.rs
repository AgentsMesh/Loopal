use loopal_message::{ContentBlock, ImageSource, Message, MessageRole};

#[test]
fn test_user_message() {
    let msg = Message::user("hello");
    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.text_content(), "hello");
}

#[test]
fn test_assistant_message() {
    let msg = Message::assistant("hi");
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.text_content(), "hi");
}

#[test]
fn test_system_message() {
    let msg = Message::system("sys");
    assert_eq!(msg.role, MessageRole::System);
    assert_eq!(msg.text_content(), "sys");
}

#[test]
fn test_text_content_ignores_non_text_blocks() {
    let msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![
            ContentBlock::Text { text: "a".into() },
            ContentBlock::ToolUse {
                id: "1".into(),
                name: "t".into(),
                input: serde_json::json!({}),
            },
            ContentBlock::Text { text: "b".into() },
        ],
    };
    assert_eq!(msg.text_content(), "ab");
}

#[test]
fn test_text_content_empty() {
    let msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![],
    };
    assert_eq!(msg.text_content(), "");
}

#[test]
fn test_estimated_token_count_text_only() {
    let msg = Message::user("abcdefgh"); // 8 chars / 4 = 2 + 4 overhead = 6
    assert_eq!(msg.estimated_token_count(), 6);
}

#[test]
fn test_estimated_token_count_tool_use() {
    let msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/test.rs"}),
        }],
    };
    // JSON serialization of input + overhead
    let expected = serde_json::json!({"file_path": "/tmp/test.rs"})
        .to_string()
        .len() as u32
        / 4
        + 4;
    assert_eq!(msg.estimated_token_count(), expected);
}

#[test]
fn test_estimated_token_count_tool_result() {
    let msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "1".into(),
            content: "a".repeat(400),
            is_error: false,
            is_completion: false,
            metadata: None,
        }],
    };
    // 400 / 4 = 100 + 4 overhead
    assert_eq!(msg.estimated_token_count(), 104);
}

#[test]
fn test_estimated_token_count_image() {
    let msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Image {
            source: ImageSource {
                source_type: "base64".into(),
                media_type: "image/png".into(),
                data: "abc".into(),
            },
        }],
    };
    // 1000 fixed + 4 overhead
    assert_eq!(msg.estimated_token_count(), 1004);
}

#[test]
fn test_estimated_token_count_mixed() {
    let msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![
            ContentBlock::Text {
                text: "abcdefgh".into(),
            }, // 8/4 = 2
            ContentBlock::ToolUse {
                id: "1".into(),
                name: "Read".into(),
                input: serde_json::json!({}), // "{}" = 2 chars / 4 = 0
            },
        ],
    };
    assert_eq!(msg.estimated_token_count(), 2 + 4);
}
