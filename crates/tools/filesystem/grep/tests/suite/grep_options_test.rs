use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_grep::GrepTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend =
        loopal_backend::LocalBackend::new(cwd.to_path_buf(), None, Default::default());
    ToolContext { backend, session_id: "test".into(), shared: None, pending_cwd_switch: Default::default(), memory_channel: None }
}

fn make_file(dir: &std::path::Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

#[tokio::test]
async fn case_insensitive_matches() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", "Hello World\nhello world\nHELLO WORLD");
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "hello", "-i": true, "output_mode": "content"}), &ctx)
        .await.unwrap();
    assert!(r.content.contains("Hello World"));
    assert!(r.content.contains("hello world"));
    assert!(r.content.contains("HELLO WORLD"));
}

#[tokio::test]
async fn case_insensitive_default_is_sensitive() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", "Hello World\nhello world");
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "hello", "output_mode": "content"}), &ctx)
        .await.unwrap();
    assert!(r.content.contains("hello world"));
    assert!(!r.content.contains("Hello World"));
}

#[tokio::test]
async fn multiline_matches_across_lines() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", "start\nhello\nworld\nend");
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "hello.world", "multiline": true, "output_mode": "content"}),
            &ctx,
        )
        .await.unwrap();
    // Both lines 2 and 3 should appear as matches
    assert!(r.content.contains("hello"));
    assert!(r.content.contains("world"));
}

#[tokio::test]
async fn type_filter_rust_only() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "code.rs", "fn main() {}");
    make_file(tmp.path(), "script.py", "fn main() {}");
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "fn main", "type": "rust", "output_mode": "content"}),
            &ctx,
        )
        .await.unwrap();
    assert!(r.content.contains("code.rs"));
    assert!(!r.content.contains("script.py"));
}

#[tokio::test]
async fn type_filter_unknown_returns_no_results() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "code.rs", "fn main() {}");
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "fn main", "type": "brainfuck"}), &ctx)
        .await.unwrap();
    assert!(r.content.contains("No matches found"));
}

#[tokio::test]
async fn offset_skips_results() {
    let tmp = tempfile::tempdir().unwrap();
    let lines: String = (0..10).map(|i| format!("match_{i}\n")).collect();
    make_file(tmp.path(), "f.txt", &lines);
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "match_", "output_mode": "content", "offset": 3, "head_limit": 3}),
            &ctx,
        )
        .await.unwrap();
    assert!(r.content.contains("match_3"));
    assert!(r.content.contains("match_5"));
    assert!(!r.content.contains("match_0"));
    assert!(!r.content.contains("match_2"));
}

#[tokio::test]
async fn offset_with_head_limit_pagination() {
    let tmp = tempfile::tempdir().unwrap();
    let lines: String = (0..20).map(|i| format!("line_{i}\n")).collect();
    make_file(tmp.path(), "f.txt", &lines);
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "line_", "output_mode": "content", "offset": 5, "head_limit": 5}),
            &ctx,
        )
        .await.unwrap();
    // Should show pagination hint
    assert!(r.content.contains("offset=10"));
}

#[tokio::test]
async fn line_numbers_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", "hello world\ngoodbye world");
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "hello", "output_mode": "content", "-n": false}),
            &ctx,
        )
        .await.unwrap();
    // Should not have :1: line number prefix
    assert!(!r.content.contains(":1:"));
    assert!(r.content.contains("hello world"));
}
