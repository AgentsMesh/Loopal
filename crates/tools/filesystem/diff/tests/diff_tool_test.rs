use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_diff::DiffTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        session_id: "test".into(),
        shared: None,
        pending_cwd_switch: Default::default(),
        backend,
    }
}

#[tokio::test]
async fn diff_two_files_basic() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello\nworld\n").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "hello\nearth\n").unwrap();
    let tool = DiffTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"path_a": "a.txt", "path_b": "b.txt"}), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("-world"));
    assert!(r.content.contains("+earth"));
}

#[tokio::test]
async fn diff_identical_files() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "same\n").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "same\n").unwrap();
    let tool = DiffTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"path_a": "a.txt", "path_b": "b.txt"}), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("No differences"));
}

#[tokio::test]
async fn diff_file_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "x").unwrap();
    let tool = DiffTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"path_a": "a.txt", "path_b": "missing.txt"}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("not found") || r.content.contains("failed to read"),
            "unexpected: {}", r.content);
}

#[tokio::test]
async fn diff_missing_params() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = DiffTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({}), &ctx).await.unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("path_a") || r.content.contains("Provide"));
}

#[tokio::test]
async fn diff_two_files_path_b_required() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "x").unwrap();
    let tool = DiffTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"path_a": "a.txt"}), &ctx).await;
    assert!(r.is_err() || r.as_ref().is_ok_and(|r| r.is_error));
}
