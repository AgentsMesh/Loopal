use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::projection::project_messages;

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
