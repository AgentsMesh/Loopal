//! Tests for UserContent: constructors, image queries, text preview, equality.

use loopal_protocol::{ImageAttachment, UserContent};

fn sample_image() -> ImageAttachment {
    ImageAttachment {
        media_type: "image/png".to_string(),
        data: "iVBORw0KGgo=".to_string(),
    }
}

#[test]
fn test_from_string() {
    let content = UserContent::from("hello".to_string());
    assert_eq!(content.text, "hello");
    assert!(content.images.is_empty());
}

#[test]
fn test_from_str() {
    let content = UserContent::from("hi");
    assert_eq!(content.text, "hi");
    assert!(content.images.is_empty());
}

#[test]
fn test_text_only() {
    let content = UserContent::text_only("x");
    assert_eq!(content.text, "x");
    assert!(content.images.is_empty());
}

#[test]
fn test_has_images() {
    let text_only = UserContent::text_only("no images");
    assert!(!text_only.has_images());

    let with_images = UserContent {
        text: "has image".to_string(),
        images: vec![sample_image()],
    };
    assert!(with_images.has_images());
}

#[test]
fn test_text_preview_short() {
    let content = UserContent::text_only("short text");
    assert_eq!(content.text_preview(), "short text");
}

#[test]
fn test_text_preview_truncates() {
    let long = "a".repeat(200);
    let content = UserContent::text_only(long);
    assert_eq!(content.text_preview().len(), 80);
}

#[test]
fn test_text_preview_multibyte() {
    // Each CJK char is 3 bytes in UTF-8. 27 chars = 81 bytes, exceeds 80.
    let cjk = "中".repeat(27);
    let preview = UserContent::text_only(&cjk).text_preview().to_string();
    // Must not split mid-char: should truncate to 26 chars (78 bytes)
    assert!(preview.len() <= 80);
    assert!(preview.is_char_boundary(preview.len()));
    assert_eq!(preview, "中".repeat(26));
}

#[test]
fn test_partial_eq() {
    let a = UserContent {
        text: "same".to_string(),
        images: vec![sample_image()],
    };
    let b = UserContent {
        text: "same".to_string(),
        images: vec![sample_image()],
    };
    assert_eq!(a, b);

    let c = UserContent::text_only("different");
    assert_ne!(a, c);
}
