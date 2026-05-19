use loopal_tool_api::{ImageResult, ToolResult};
use loopal_tool_invocation::ToolImageBlock;

#[test]
fn test_tool_result_success() {
    let r = ToolResult::success("ok");
    assert_eq!(r.content, "ok");
    assert!(!r.is_error);
    assert!(r.images.is_empty());
}

#[test]
fn test_tool_result_error() {
    let r = ToolResult::error("fail");
    assert_eq!(r.content, "fail");
    assert!(r.is_error);
    assert!(r.images.is_empty());
}

#[test]
fn test_tool_result_success_from_string() {
    let r = ToolResult::success(String::from("hello"));
    assert_eq!(r.content, "hello");
    assert!(!r.is_error);
}

#[test]
fn test_with_image_appends_inline_block() {
    let img = ImageResult {
        media_type: "image/png".into(),
        data: "iVBORw0KGgo".into(),
        dimensions: (64, 64),
        byte_size: 1024,
    };
    let r = ToolResult::success("loaded").with_image(img);
    assert_eq!(r.images.len(), 1);
    match &r.images[0] {
        ToolImageBlock::Inline { media_type, data } => {
            assert_eq!(media_type, "image/png");
            assert_eq!(data, "iVBORw0KGgo");
        }
        _ => panic!("expected Inline"),
    }
}

#[test]
fn test_with_images_extends() {
    let imgs = vec![
        ToolImageBlock::inline("image/png", "a"),
        ToolImageBlock::session_resource("hash1", "image/jpeg", 100),
    ];
    let r = ToolResult::success("").with_images(imgs);
    assert_eq!(r.images.len(), 2);
}

#[test]
fn test_serialization_skips_empty_images() {
    let r = ToolResult::success("plain");
    let v = serde_json::to_value(&r).unwrap();
    assert!(v.get("images").is_none(), "empty images must skip");
}

#[test]
fn test_serialization_emits_images_when_non_empty() {
    let r = ToolResult::success("ok").with_image(ImageResult {
        media_type: "image/png".into(),
        data: "AAAA".into(),
        dimensions: (1, 1),
        byte_size: 4,
    });
    let v = serde_json::to_value(&r).unwrap();
    assert!(v["images"].is_array());
    assert_eq!(v["images"][0]["type"], "inline");
}

#[test]
fn test_legacy_json_deserializes_without_images() {
    let legacy = r#"{"content":"ok","is_error":false}"#;
    let r: ToolResult = serde_json::from_str(legacy).unwrap();
    assert_eq!(r.content, "ok");
    assert!(r.images.is_empty());
}
