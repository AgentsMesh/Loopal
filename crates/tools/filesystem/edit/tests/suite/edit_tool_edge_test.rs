use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_edit::EditTool;
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
        pending_cwd_switch: Default::default(), memory_channel: None,
        backend,
    }
}

#[tokio::test]
async fn test_edit_replace_all_with_multiple_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("multi.txt");
    std::fs::write(&file, "foo bar foo baz foo").unwrap();

    let tool = EditTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "old_string": "foo",
                "new_string": "qux",
                "replace_all": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("Successfully edited"));

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "qux bar qux baz qux");
}

#[tokio::test]
async fn test_edit_nonexistent_file_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("does_not_exist.txt");

    let tool = EditTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "old_string": "abc",
                "new_string": "def"
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Backend returns ToolIoError::NotFound, which is caught as ToolResult::error
    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}

#[tokio::test]
async fn test_edit_missing_file_path_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = EditTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "old_string": "a",
                "new_string": "b"
            }),
            &ctx,
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_edit_missing_old_string_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.txt");
    std::fs::write(&file, "content").unwrap();

    let tool = EditTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "new_string": "b"
            }),
            &ctx,
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_edit_missing_new_string_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.txt");
    std::fs::write(&file, "content").unwrap();

    let tool = EditTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "old_string": "content"
            }),
            &ctx,
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_edit_with_relative_path() {
    let tmp = tempfile::tempdir().unwrap();
    // Canonicalize to handle macOS /tmp -> /private/tmp symlink
    let canon = tmp.path().canonicalize().unwrap();
    let file = canon.join("relative.txt");
    std::fs::write(&file, "old content").unwrap();

    let tool = EditTool;
    let ctx = make_ctx(&canon);

    let result = tool
        .execute(
            json!({
                "file_path": "relative.txt",
                "old_string": "old content",
                "new_string": "new content"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "new content");
}
