use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_read::ReadTool;
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
        output_tail: None,
        backend,
    }
}

#[tokio::test]
async fn test_read_existing_file_returns_content() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("hello.txt");
    std::fs::write(&file, "line one\nline two\nline three").unwrap();

    let tool = ReadTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("line one"));
    assert!(result.content.contains("line two"));
    assert!(result.content.contains("line three"));
    // Output should include line numbers (cat -n format)
    assert!(result.content.contains("1\t"));
}

#[tokio::test]
async fn test_read_nonexistent_file_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("does_not_exist.txt");

    let tool = ReadTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    // Backend returns ToolIoError::NotFound → ToolResult::error
    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}

#[tokio::test]
async fn test_read_with_line_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("lines.txt");
    let content = (1..=10)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&file, &content).unwrap();

    let tool = ReadTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({"file_path": file.to_str().unwrap(), "limit": 3}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    // Should only have 3 lines
    let output_lines: Vec<&str> = result.content.trim().lines().collect();
    assert_eq!(output_lines.len(), 3);
    assert!(result.content.contains("line 1"));
    assert!(result.content.contains("line 3"));
    assert!(!result.content.contains("line 4"));
}

#[tokio::test]
async fn test_read_with_offset() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("lines.txt");
    let content = (1..=5)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&file, &content).unwrap();

    let tool = ReadTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({"file_path": file.to_str().unwrap(), "offset": 3, "limit": 2}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let output_lines: Vec<&str> = result.content.trim().lines().collect();
    assert_eq!(output_lines.len(), 2);
    assert!(result.content.contains("line 3"));
    assert!(result.content.contains("line 4"));
    assert!(!result.content.contains("line 2"));
}

#[test]
fn test_read_name() {
    let tool = ReadTool;
    assert_eq!(tool.name(), "Read");
}

#[test]
fn test_read_description() {
    let tool = ReadTool;
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Read"));
}

#[test]
fn test_read_permission() {
    let tool = ReadTool;
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_read_parameters_schema() {
    let tool = ReadTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("file_path")));
    assert!(schema["properties"]["file_path"].is_object());
    assert!(schema["properties"]["offset"].is_object());
    assert!(schema["properties"]["limit"].is_object());
}

#[tokio::test]
async fn test_read_empty_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("empty.txt");
    std::fs::write(&file, "").unwrap();

    let tool = ReadTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    // Empty file produces empty output
    assert!(result.content.is_empty() || result.content.trim().is_empty());
}

#[tokio::test]
async fn test_read_missing_file_path_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = ReadTool;
    let ctx = make_ctx(tmp.path());

    let result = tool.execute(json!({}), &ctx).await;

    assert!(result.is_err());
}
