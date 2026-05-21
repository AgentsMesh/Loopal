use loopal_context::token_counter::estimate_message_tokens;
use loopal_message::{ContentBlock, ImageSource, Message, MessageRole};

const IMAGE_CAP: u32 = 2000;
const FRAMING_OVERHEAD: u32 = 4;

fn image_block(data: &str) -> ContentBlock {
    ContentBlock::Image {
        source: ImageSource {
            source_type: "base64".into(),
            media_type: "image/png".into(),
            data: data.into(),
        },
    }
}

fn user_msg(content: Vec<ContentBlock>) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content,
        origin: None,
        ephemeral_in_history: false,
    }
}

#[test]
fn image_block_caps_at_2000_tokens_plus_framing() {
    let msg = user_msg(vec![image_block("iVBORw0KGgoAAAANSUhEUgAA")]);
    assert_eq!(estimate_message_tokens(&msg), IMAGE_CAP + FRAMING_OVERHEAD);
}

#[test]
fn multiple_image_blocks_accumulate_linearly() {
    let msg = user_msg(vec![image_block("a"), image_block("b"), image_block("c")]);
    assert_eq!(
        estimate_message_tokens(&msg),
        IMAGE_CAP * 3 + FRAMING_OVERHEAD,
        "three images must accumulate as 3×{IMAGE_CAP}+framing",
    );
}

#[test]
fn image_plus_text_combines_correctly() {
    let text_only = user_msg(vec![ContentBlock::Text {
        text: "hello world".into(),
    }]);
    let text_tokens = estimate_message_tokens(&text_only) - FRAMING_OVERHEAD;

    let mixed = user_msg(vec![
        image_block("data"),
        ContentBlock::Text {
            text: "hello world".into(),
        },
    ]);
    assert_eq!(
        estimate_message_tokens(&mixed),
        IMAGE_CAP + text_tokens + FRAMING_OVERHEAD,
        "mixed image+text must equal image_cap + text_tokens + framing",
    );
}

#[test]
fn image_cap_is_independent_of_base64_payload_size() {
    let tiny = user_msg(vec![image_block("a")]);
    let huge = user_msg(vec![image_block(&"x".repeat(1_000_000))]);
    assert_eq!(
        estimate_message_tokens(&tiny),
        estimate_message_tokens(&huge),
        "image cap must not scale with base64 data length",
    );
}

#[test]
fn image_cap_never_exceeds_upper_bound() {
    let msg = user_msg(vec![image_block("data")]);
    assert!(
        estimate_message_tokens(&msg) <= IMAGE_CAP + FRAMING_OVERHEAD,
        "single image must not exceed cap+framing — guards against accidental scaling",
    );
}
