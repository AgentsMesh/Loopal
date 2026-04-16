use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::projection::project_messages;

#[test]
fn summarize_input_respects_utf8_boundary() {
    // Exact reproduction of the crash: byte 57 falls inside '建' (bytes 55..58).
    let long_chinese = serde_json::json!({
        "subject": "创建 content-pipeline 目录结构",
        "description": "创建 channels/content-pipeline/drafts/ 目录和 .gitkeep",
        "activeForm": "Creating directories"
    });
    let raw = long_chinese.to_string();
    assert!(raw.len() > 60, "input must exceed 60 bytes to trigger truncation");
    assert!(!raw.is_char_boundary(57), "byte 57 must be mid-character to test the fix");

    let msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "tu-utf8".into(),
            name: "TaskCreate".into(),
            input: long_chinese,
        }],
    };
    let display = project_messages(&[msg]);
    let summary = &display[0].tool_calls[0].summary;
    assert!(summary.starts_with("TaskCreate("));
    assert!(summary.ends_with("...)"));
    // Verify truncated content is valid UTF-8 (iterating chars would panic otherwise)
    assert!(summary.chars().count() > 0);
}

#[test]
fn summarize_input_short_input_not_truncated() {
    let short = serde_json::json!({"path": "/tmp/foo"});
    let msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "tu-short".into(),
            name: "Read".into(),
            input: short.clone(),
        }],
    };
    let display = project_messages(&[msg]);
    let summary = &display[0].tool_calls[0].summary;
    assert_eq!(summary, &format!("Read({})", short));
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
