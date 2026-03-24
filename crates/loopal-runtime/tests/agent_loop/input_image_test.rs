//! Tests for build_user_message image handling via wait_for_input.

use loopal_message::ContentBlock;
use loopal_protocol::{Envelope, ImageAttachment, MessageSource, UserContent};

use super::make_runner_with_channels;

fn sample_image(label: &str) -> ImageAttachment {
    ImageAttachment {
        media_type: "image/png".to_string(),
        data: format!("base64-{label}"),
    }
}

#[tokio::test]
async fn test_wait_for_input_with_images() {
    let (mut runner, _event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    let content = UserContent {
        text: "describe these".to_string(),
        images: vec![sample_image("a"), sample_image("b")],
    };
    mbox_tx
        .send(Envelope::new(MessageSource::Human, "main", content))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());

    let msg = &runner.params.store.messages()[0];
    // Text block + 2 Image blocks
    assert_eq!(msg.content.len(), 3);
    assert!(matches!(&msg.content[0], ContentBlock::Text { text } if text == "describe these"));
    assert!(matches!(&msg.content[1], ContentBlock::Image { .. }));
    assert!(matches!(&msg.content[2], ContentBlock::Image { .. }));
}

#[tokio::test]
async fn test_wait_for_input_empty_text_with_images() {
    let (mut runner, _event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    let content = UserContent {
        text: String::new(),
        images: vec![sample_image("only")],
    };
    mbox_tx
        .send(Envelope::new(MessageSource::Human, "main", content))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());

    let msg = &runner.params.store.messages()[0];
    // Empty text is skipped; only 1 Image block
    assert_eq!(msg.content.len(), 1);
    assert!(matches!(&msg.content[0], ContentBlock::Image { .. }));
}

#[tokio::test]
async fn test_wait_for_input_text_only_no_images() {
    let (mut runner, _event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    mbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "just text"))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(result.is_some());

    let msg = &runner.params.store.messages()[0];
    // Only 1 Text block, no images
    assert_eq!(msg.content.len(), 1);
    assert!(matches!(&msg.content[0], ContentBlock::Text { .. }));
}
