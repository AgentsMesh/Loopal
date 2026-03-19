use loopal_types::tool::{Tool, ToolContext};
use loopal_tools::builtin::write::WriteTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    ToolContext {
        cwd: cwd.to_path_buf(),
        session_id: "test".into(),
        shared: None,
    }
}

#[tokio::test]
async fn test_write_missing_content_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("no_content.txt");

    let tool = WriteTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({"file_path": file.to_str().unwrap()}),
            &ctx,
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_write_reports_byte_count() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("bytes.txt");

    let tool = WriteTool;
    let ctx = make_ctx(tmp.path());

    let content = "hello world";
    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "content": content
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains(&format!("{} bytes", content.len())));
}

#[tokio::test]
async fn test_write_with_relative_path() {
    let tmp = tempfile::tempdir().unwrap();
    // Canonicalize to handle macOS /tmp -> /private/tmp symlink
    let canon = tmp.path().canonicalize().unwrap();
    let tool = WriteTool;
    let ctx = make_ctx(&canon);

    let result = tool
        .execute(
            json!({
                "file_path": "relative_file.txt",
                "content": "relative write"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let content = std::fs::read_to_string(canon.join("relative_file.txt")).unwrap();
    assert_eq!(content, "relative write");
}

#[tokio::test]
async fn test_write_absolute_path_bypasses_traversal_check() {
    // L62: absolute path skips the traversal check entirely (is_absolute() is true)
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("absolute_test.txt");

    let tool = WriteTool;
    // cwd is different from where we write, but since path is absolute, it's allowed
    let ctx = make_ctx(std::path::Path::new("/"));

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "content": "absolute path write"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "absolute path write");
}

#[tokio::test]
async fn test_write_relative_path_existing_file_within_cwd() {
    // L64: path.exists() is true, canonicalize check
    let tmp = tempfile::tempdir().unwrap();
    let canon = tmp.path().canonicalize().unwrap();
    let file = canon.join("existing.txt");
    std::fs::write(&file, "original").unwrap();

    let tool = WriteTool;
    let ctx = make_ctx(&canon);

    let result = tool
        .execute(
            json!({
                "file_path": "existing.txt",
                "content": "overwritten"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "overwritten");
}

#[tokio::test]
async fn test_write_relative_new_file_parent_doesnt_exist() {
    // L68: path doesn't exist AND parent doesn't exist
    // When both the path and parent don't exist, check_path is None.
    // In that case, the if-let at L75 doesn't match, so the write proceeds.
    let tmp = tempfile::tempdir().unwrap();
    let canon = tmp.path().canonicalize().unwrap();

    let tool = WriteTool;
    let ctx = make_ctx(&canon);

    // "nonexistent_dir/file.txt" relative to cwd
    let result = tool
        .execute(
            json!({
                "file_path": "deep/nested/new_dir/file.txt",
                "content": "new nested file"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let content = std::fs::read_to_string(canon.join("deep/nested/new_dir/file.txt")).unwrap();
    assert_eq!(content, "new nested file");
}
