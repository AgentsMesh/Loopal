use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_file_ops::delete::DeleteTool;
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
async fn delete_file() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("f.txt"), "x").unwrap();
    let tool = DeleteTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"path": "f.txt"}), &ctx).await.unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(r.content.contains("file"));
    assert!(!tmp.path().join("f.txt").exists());
}

#[tokio::test]
async fn delete_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("subdir");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("a.txt"), "a").unwrap();
    std::fs::write(sub.join("b.txt"), "b").unwrap();
    let tool = DeleteTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"path": "subdir"}), &ctx).await.unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(r.content.contains("directory"));
    assert!(!sub.exists());
}

#[tokio::test]
async fn delete_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = DeleteTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"path": "nope.txt"}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(
        r.content.contains("not found") || r.content.contains("does not exist"),
        "unexpected error: {}",
        r.content
    );
}

#[tokio::test]
async fn delete_path_traversal_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = DeleteTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"path": "/tmp"}), &ctx).await;
    assert!(
        r.is_err() || r.as_ref().is_ok_and(|r| r.is_error),
        "path traversal should be rejected"
    );
}
