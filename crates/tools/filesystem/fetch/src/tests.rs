use loopal_tool_api::TypedTool;

use super::{FetchTool, extension_from_content_type, is_success, simple_uuid};

#[test]
fn success_status_is_strictly_two_hundred_range() {
    assert!(!is_success(199));
    assert!(is_success(200));
    assert!(is_success(299));
    assert!(!is_success(300));
}

#[test]
fn content_types_map_to_stable_extensions() {
    for (content_type, extension) in [
        ("text/html; charset=utf-8", "html"),
        ("application/pdf", "pdf"),
        ("image/png", "png"),
        ("image/jpeg", "jpg"),
        ("image/svg+xml", "svg"),
        ("application/json", "json"),
        ("text/plain", "txt"),
        ("application/octet-stream", "bin"),
    ] {
        assert_eq!(extension_from_content_type(content_type), extension);
    }
}

#[test]
fn generated_file_name_token_is_hex() {
    let token = simple_uuid();
    assert_eq!(token.len(), 16);
    assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[test]
fn fetch_accepts_no_secret_parameters() {
    assert!(FetchTool.secret_eligible_params().is_empty());
}
