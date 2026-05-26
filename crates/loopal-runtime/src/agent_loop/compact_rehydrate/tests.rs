use super::RehydrateStats;
use super::helpers::trim_body;
use loopal_tool_api::ToolResult;

fn result(s: &str) -> ToolResult {
    ToolResult {
        content: s.to_string(),
        images: Vec::new(),
        is_error: false,
        metadata: None,
    }
}

#[test]
fn trim_body_passthrough_under_cap() {
    let out = trim_body(result("hello"), 1024);
    assert_eq!(out, "hello");
}

#[test]
fn trim_body_truncates_with_marker() {
    let body = "x".repeat(100);
    let out = trim_body(result(&body), 10);
    assert!(out.starts_with(&"x".repeat(10)));
    assert!(out.contains("90 bytes truncated"));
}

#[test]
fn trim_body_respects_char_boundary() {
    let out = trim_body(result("你好啊"), 4);
    assert!(out.starts_with("你"));
    assert!(!out.contains("好啊"));
}

#[test]
fn rehydrate_stats_default_is_zero() {
    let s = RehydrateStats::default();
    assert_eq!(s.files_attempted, 0);
    assert_eq!(s.files_succeeded, 0);
    assert_eq!(s.bytes_injected, 0);
}
