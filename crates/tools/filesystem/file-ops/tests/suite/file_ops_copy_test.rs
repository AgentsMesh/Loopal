use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_file_ops::copy::CopyFileTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test")
}

#[tokio::test]
async fn copy_basic() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let tool = CopyFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "a.txt", "dst": "b.txt"}), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(r.content.contains("Copied"));
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
        "hello"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("b.txt")).unwrap(),
        "hello"
    );
}

#[tokio::test]
async fn copy_src_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = CopyFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "nope.txt", "dst": "b.txt"}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
}

#[tokio::test]
async fn copy_dst_is_directory() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "data").unwrap();
    std::fs::create_dir(tmp.path().join("dest")).unwrap();
    let tool = CopyFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "a.txt", "dst": "dest"}), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(tmp.path().join("dest/a.txt").exists());
}

#[tokio::test]
async fn copy_creates_parent_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "data").unwrap();
    let tool = CopyFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "a.txt", "dst": "deep/nested/b.txt"}), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(tmp.path().join("deep/nested/b.txt").exists());
}

#[tokio::test]
async fn copy_path_traversal_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "x").unwrap();
    let tool = CopyFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "a.txt", "dst": "/tmp/outside.txt"}), &ctx)
        .await;
    assert!(
        r.is_err() || r.as_ref().is_ok_and(|r| r.is_error),
        "path traversal should be rejected"
    );
}
