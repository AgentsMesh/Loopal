use loopagent_types::tool::{Tool, ToolContext};
use loopagent_tools::builtin::write::WriteTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    ToolContext {
        cwd: cwd.to_path_buf(),
        session_id: "test".into(),
    }
}

#[tokio::test]
async fn test_write_omission_in_content_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.rs");

    let tool = WriteTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "content": "fn main() {\n    // ... rest of code\n}"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Omission detected"));
    // File should not have been created
    assert!(!file.exists());
}

#[tokio::test]
async fn test_write_path_traversal_protection() {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().join("inner");
    std::fs::create_dir_all(&cwd).unwrap();

    let tool = WriteTool;
    let ctx = make_ctx(&cwd);

    let result = tool
        .execute(
            json!({
                "file_path": "../outside.txt",
                "content": "should not be written"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("path outside working directory"));

    // Verify file was not created outside cwd
    assert!(!tmp.path().join("outside.txt").exists());
}

#[tokio::test]
async fn test_write_missing_file_path_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = WriteTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"content": "something"}), &ctx)
        .await;

    assert!(result.is_err());
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
