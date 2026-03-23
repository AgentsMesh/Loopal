use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_ls::LsTool;
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

#[test]
fn test_ls_name() {
    let tool = LsTool;
    assert_eq!(tool.name(), "Ls");
}

#[test]
fn test_ls_description() {
    let tool = LsTool;
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("directory"));
}

#[test]
fn test_ls_permission() {
    let tool = LsTool;
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_ls_parameters_schema() {
    let tool = LsTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["path"].is_object());
}

#[tokio::test]
async fn test_ls_directory_with_files() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("alpha.txt"), "a").unwrap();
    std::fs::write(tmp.path().join("beta.txt"), "b").unwrap();
    std::fs::create_dir(tmp.path().join("subdir")).unwrap();

    let tool = LsTool;
    let ctx = make_ctx(tmp.path());

    let result = tool.execute(json!({}), &ctx).await.unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("alpha.txt"));
    assert!(result.content.contains("beta.txt"));
    // Directories should have trailing /
    assert!(result.content.contains("subdir/"));
}

#[tokio::test]
async fn test_ls_empty_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());

    let result = tool.execute(json!({}), &ctx).await.unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("(empty directory)"));
}

#[tokio::test]
async fn test_ls_nonexistent_path_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({"path": "/nonexistent/path/that/does/not/exist"}),
            &ctx,
        )
        .await
        .unwrap();

    // Backend returns ToolIoError -> ToolResult::error
    assert!(result.is_error);
}

#[tokio::test]
async fn test_ls_with_explicit_absolute_path() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("mydir");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("inner.txt"), "inner").unwrap();

    let tool = LsTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"path": sub.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("inner.txt"));
}

#[tokio::test]
async fn test_ls_with_relative_path() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("reldir");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("rel.txt"), "relative").unwrap();

    let tool = LsTool;
    let ctx = make_ctx(tmp.path());

    let result = tool.execute(json!({"path": "reldir"}), &ctx).await.unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("rel.txt"));
}

#[tokio::test]
async fn test_ls_entries_are_sorted() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("zebra.txt"), "z").unwrap();
    std::fs::write(tmp.path().join("apple.txt"), "a").unwrap();
    std::fs::write(tmp.path().join("mango.txt"), "m").unwrap();

    let tool = LsTool;
    let ctx = make_ctx(tmp.path());

    let result = tool.execute(json!({}), &ctx).await.unwrap();

    assert!(!result.is_error);
    let lines: Vec<&str> = result.content.lines().collect();
    assert_eq!(lines[0], "apple.txt");
    assert_eq!(lines[1], "mango.txt");
    assert_eq!(lines[2], "zebra.txt");
}

#[tokio::test]
async fn test_ls_default_uses_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("cwdfile.txt"), "cwd").unwrap();

    let tool = LsTool;
    let ctx = make_ctx(tmp.path());

    // No path parameter means use cwd
    let result = tool.execute(json!({}), &ctx).await.unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("cwdfile.txt"));
}
