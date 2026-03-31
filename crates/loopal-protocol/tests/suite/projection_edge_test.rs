use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::projection::project_messages;

#[test]
fn project_attempt_completion_promoted_to_assistant_message() {
    let assistant_msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "tc-comp".into(),
            name: "AttemptCompletion".into(),
            input: serde_json::json!({"result": "All done"}),
        }],
    };
    let user_msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tc-comp".into(),
            content: "All done".into(),
            is_error: false,
            is_completion: true,
            metadata: None,
        }],
    };
    let display = project_messages(&[assistant_msg, user_msg]);
    // ToolCall should be status "success" but NO result stored
    assert_eq!(display[0].tool_calls[0].name, "AttemptCompletion");
    assert!(!display[0].tool_calls[0].is_error);
    assert!(
        display[0].tool_calls[0].result.is_none(),
        "AttemptCompletion should not store result in tool call"
    );
    // Completion should be promoted to a separate assistant message
    assert_eq!(display.len(), 2);
    assert_eq!(display[1].role, "assistant");
    assert_eq!(display[1].content, "All done");
    assert!(display[1].tool_calls.is_empty());
}

#[test]
fn project_attempt_completion_error_not_promoted() {
    let assistant_msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "tc-fail".into(),
            name: "AttemptCompletion".into(),
            input: serde_json::json!({"result": "oops"}),
        }],
    };
    let user_msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tc-fail".into(),
            content: "completion failed".into(),
            is_error: true,
            is_completion: false,
            metadata: None,
        }],
    };
    let display = project_messages(&[assistant_msg, user_msg]);
    // Error should NOT be promoted — treated as normal tool error
    assert_eq!(display.len(), 1);
    assert!(display[0].tool_calls[0].is_error);
    assert!(display[0].tool_calls[0].result.is_some());
}

#[test]
fn project_multiple_images_count() {
    let msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![
            ContentBlock::Text {
                text: "check these".into(),
            },
            ContentBlock::Image {
                source: loopal_message::ImageSource {
                    source_type: "base64".into(),
                    media_type: "image/png".into(),
                    data: "img1".into(),
                },
            },
            ContentBlock::Image {
                source: loopal_message::ImageSource {
                    source_type: "base64".into(),
                    media_type: "image/jpeg".into(),
                    data: "img2".into(),
                },
            },
            ContentBlock::Image {
                source: loopal_message::ImageSource {
                    source_type: "base64".into(),
                    media_type: "image/png".into(),
                    data: "img3".into(),
                },
            },
        ],
    };
    let display = project_messages(&[msg]);
    assert_eq!(display.len(), 1);
    assert_eq!(display[0].image_count, 3);
    // Text content should include "[image]" placeholders
    assert!(display[0].content.contains("check these"));
    assert_eq!(display[0].content.matches("[image]").count(), 3);
}
