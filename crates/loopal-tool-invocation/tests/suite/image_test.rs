use loopal_tool_invocation::{ImageMime, ToolImageBlock};

#[test]
fn image_mime_from_magic_recognizes_png() {
    assert_eq!(
        ImageMime::from_magic(b"\x89PNG\r\n\x1a\nrest"),
        Some(ImageMime::Png)
    );
}

#[test]
fn image_mime_from_magic_recognizes_jpeg() {
    assert_eq!(
        ImageMime::from_magic(b"\xff\xd8\xffrest"),
        Some(ImageMime::Jpeg)
    );
}

#[test]
fn image_mime_from_magic_recognizes_gif87a_and_gif89a() {
    assert_eq!(ImageMime::from_magic(b"GIF87a..."), Some(ImageMime::Gif));
    assert_eq!(ImageMime::from_magic(b"GIF89a..."), Some(ImageMime::Gif));
}

#[test]
fn image_mime_from_magic_recognizes_webp() {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&[0; 4]);
    buf.extend_from_slice(b"WEBPrest");
    assert_eq!(ImageMime::from_magic(&buf), Some(ImageMime::Webp));
}

#[test]
fn image_mime_from_magic_returns_none_for_unknown() {
    assert_eq!(ImageMime::from_magic(b"hello world!"), None);
}

#[test]
fn image_mime_from_magic_returns_none_for_truncated_bytes() {
    assert_eq!(ImageMime::from_magic(b"\x89PNG"), None);
    assert_eq!(ImageMime::from_magic(b"\xff\xd8"), None);
    assert_eq!(ImageMime::from_magic(b"GIF8"), None);
    assert_eq!(ImageMime::from_magic(b"RIFF\x00\x00\x00\x00WEB"), None);
}

#[test]
fn image_mime_from_magic_returns_none_for_empty() {
    assert_eq!(ImageMime::from_magic(b""), None);
}

#[test]
fn image_mime_from_mime_str_recognizes_all_formats() {
    assert_eq!(ImageMime::from_mime_str("image/png"), Some(ImageMime::Png));
    assert_eq!(
        ImageMime::from_mime_str("image/jpeg"),
        Some(ImageMime::Jpeg)
    );
    assert_eq!(ImageMime::from_mime_str("image/gif"), Some(ImageMime::Gif));
    assert_eq!(
        ImageMime::from_mime_str("image/webp"),
        Some(ImageMime::Webp)
    );
}

#[test]
fn image_mime_from_mime_str_returns_none_for_unsupported() {
    assert_eq!(ImageMime::from_mime_str("image/bmp"), None);
    assert_eq!(ImageMime::from_mime_str("application/octet-stream"), None);
    assert_eq!(ImageMime::from_mime_str(""), None);
    assert_eq!(ImageMime::from_mime_str("IMAGE/PNG"), None); // case-sensitive
}

#[test]
fn image_mime_as_str_round_trip() {
    for mime in [
        ImageMime::Png,
        ImageMime::Jpeg,
        ImageMime::Gif,
        ImageMime::Webp,
    ] {
        assert_eq!(ImageMime::from_mime_str(mime.as_str()), Some(mime));
    }
}

#[test]
fn inline_serializes_with_type_tag() {
    let blk = ToolImageBlock::inline("image/png", "iVBORw0KGgo");
    let v = serde_json::to_value(&blk).unwrap();
    assert_eq!(v["type"], "inline");
    assert_eq!(v["media_type"], "image/png");
    assert_eq!(v["data"], "iVBORw0KGgo");
}

#[test]
fn session_resource_serializes_with_type_tag() {
    let blk = ToolImageBlock::session_resource("a3f2c8b9d1e4f5a6", "image/jpeg", 512);
    let v = serde_json::to_value(&blk).unwrap();
    assert_eq!(v["type"], "session_resource");
    assert_eq!(v["id"], "a3f2c8b9d1e4f5a6");
    assert_eq!(v["media_type"], "image/jpeg");
    assert_eq!(v["byte_size"], 512);
}

#[test]
fn inline_round_trip() {
    let blk = ToolImageBlock::inline("image/gif", "R0lGODlh");
    let json = serde_json::to_string(&blk).unwrap();
    let back: ToolImageBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(blk, back);
}

#[test]
fn session_resource_round_trip() {
    let blk = ToolImageBlock::session_resource("deadbeef", "image/webp", 1024);
    let json = serde_json::to_string(&blk).unwrap();
    let back: ToolImageBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(blk, back);
}

#[test]
fn deserialize_inline_from_explicit_json() {
    let json = r#"{"type":"inline","media_type":"image/png","data":"AAAA"}"#;
    let blk: ToolImageBlock = serde_json::from_str(json).unwrap();
    assert!(blk.is_inline());
    assert_eq!(blk.media_type(), "image/png");
    assert_eq!(blk.byte_size(), 3);
}

#[test]
fn deserialize_session_resource_from_explicit_json() {
    let json =
        r#"{"type":"session_resource","id":"abc","media_type":"image/jpeg","byte_size":512}"#;
    let blk: ToolImageBlock = serde_json::from_str(json).unwrap();
    assert!(!blk.is_inline());
    assert_eq!(blk.media_type(), "image/jpeg");
    assert_eq!(blk.byte_size(), 512);
}

#[test]
fn media_type_accessor_returns_borrowed_str() {
    let inline = ToolImageBlock::inline("image/png", "x");
    let res = ToolImageBlock::session_resource("id", "image/jpeg", 16);
    assert_eq!(inline.media_type(), "image/png");
    assert_eq!(res.media_type(), "image/jpeg");
}

#[test]
fn is_inline_distinguishes_variants() {
    assert!(ToolImageBlock::inline("image/png", "x").is_inline());
    assert!(!ToolImageBlock::session_resource("id", "image/png", 16).is_inline());
}

#[test]
fn inline_byte_size_derives_from_base64_length() {
    let blk = ToolImageBlock::inline("image/png", "AAAA");
    assert_eq!(blk.byte_size(), 3);
}

#[test]
fn session_resource_byte_size_returns_stored_value() {
    let blk = ToolImageBlock::session_resource("hash", "image/png", 4096);
    assert_eq!(blk.byte_size(), 4096);
}
