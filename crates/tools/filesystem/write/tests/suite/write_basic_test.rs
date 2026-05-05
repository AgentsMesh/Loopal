use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_write::WriteTool;
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
#[cfg(not(windows))]
async fn test_write_absolute_path_bypasses_traversal_check() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("absolute_test.txt");

    let tool = WriteTool;
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
