use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_write::WriteTool;
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
        pending_cwd_switch: Default::default(), memory_channel: None, output_tail: None,
        backend,
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
    assert!(!file.exists());
}

#[tokio::test]
async fn test_write_valid_content_creates_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("output.txt");

    let tool = WriteTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "content": "hello world"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("Successfully wrote"));

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "hello world");
}

#[tokio::test]
async fn test_write_creates_parent_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a").join("b").join("c").join("file.txt");

    let tool = WriteTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "content": "nested content"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(file.exists());

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "nested content");
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
    assert!(result.content.contains("path escapes working directory"));
    assert!(!tmp.path().join("outside.txt").exists());
}

#[test]
fn test_write_name() {
    let tool = WriteTool;
    assert_eq!(tool.name(), "Write");
}

#[test]
fn test_write_description() {
    let tool = WriteTool;
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Write"));
}

#[test]
fn test_write_permission() {
    let tool = WriteTool;
    assert_eq!(tool.permission(), PermissionLevel::Supervised);
}

#[test]
fn test_write_parameters_schema() {
    let tool = WriteTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("file_path")));
    assert!(required.contains(&json!("content")));
    assert!(schema["properties"]["file_path"].is_object());
    assert!(schema["properties"]["content"].is_object());
}

#[tokio::test]
async fn test_write_overwrite_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("overwrite.txt");
    std::fs::write(&file, "original content").unwrap();

    let tool = WriteTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "content": "new content"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("Successfully wrote"));

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "new content");
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
