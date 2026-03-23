use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_file_ops::move_file::MoveFileTool;
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
        memory_channel: None,
        backend,
    }
}

#[tokio::test]
async fn move_same_directory() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let tool = MoveFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "a.txt", "dst": "b.txt"}), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(!tmp.path().join("a.txt").exists());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("b.txt")).unwrap(),
        "hello"
    );
}

#[tokio::test]
async fn move_across_directories() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "data").unwrap();
    let tool = MoveFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "a.txt", "dst": "sub/dir/a.txt"}), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(!tmp.path().join("a.txt").exists());
    assert!(tmp.path().join("sub/dir/a.txt").exists());
}

#[tokio::test]
async fn move_src_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = MoveFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "nope.txt", "dst": "b.txt"}), &ctx)
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
async fn move_dst_is_directory() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    std::fs::create_dir(tmp.path().join("dest")).unwrap();
    let tool = MoveFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "a.txt", "dst": "dest"}), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(tmp.path().join("dest/a.txt").exists());
}

#[tokio::test]
async fn move_path_traversal_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "x").unwrap();
    let tool = MoveFileTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"src": "a.txt", "dst": "/tmp/outside.txt"}), &ctx)
        .await;
    // Path traversal should be rejected either as error result or LoopalError
    assert!(
        r.is_err() || r.as_ref().is_ok_and(|r| r.is_error),
        "path traversal should be rejected"
    );
}
