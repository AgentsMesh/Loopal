use loopal_tool_api::{PermissionLevel, Tool, ToolContext, TypedBridge};
use loopal_tool_edit::{EditParams, EditTool};
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test")
}

fn make_tool() -> impl Tool {
    TypedBridge::<EditTool, EditParams>::new(EditTool)
}

#[tokio::test]
async fn test_edit_omission_in_new_string_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.rs");
    std::fs::write(&file, "fn main() {}").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "old_string": "fn main() {}",
                "new_string": "fn main() {\n    // ... rest of code\n}"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Omission detected"));
}

#[tokio::test]
async fn test_edit_valid_replacement_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("hello.txt");
    std::fs::write(&file, "hello world").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "old_string": "world",
                "new_string": "rust"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("Successfully edited"));

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "hello rust");
}

#[tokio::test]
async fn test_edit_old_string_not_found_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.txt");
    std::fs::write(&file, "hello world").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "old_string": "nonexistent",
                "new_string": "replacement"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}

#[tokio::test]
async fn test_edit_multiple_matches_without_replace_all_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.txt");
    std::fs::write(&file, "aaa").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "old_string": "a",
                "new_string": "b"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("3 times"));
    assert!(result.content.contains("replace_all"));
}

#[tokio::test]
async fn test_edit_path_traversal_relative_path() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside.txt");
    std::fs::write(&outside, "secret").unwrap();

    let cwd = tmp.path().join("inner");
    std::fs::create_dir_all(&cwd).unwrap();

    let tool = make_tool();
    let ctx = make_ctx(&cwd);

    let result = tool
        .execute(
            json!({
                "file_path": "../outside.txt",
                "old_string": "secret",
                "new_string": "hacked"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("path denied"));

    let content = std::fs::read_to_string(&outside).unwrap();
    assert_eq!(content, "secret");
}

#[test]
fn test_edit_name() {
    let tool = make_tool();
    assert_eq!(tool.name(), "Edit");
}

#[test]
fn test_edit_description() {
    let tool = make_tool();
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("replacement"));
}

#[test]
fn test_edit_permission() {
    let tool = make_tool();
    assert_eq!(tool.permission(), PermissionLevel::Write);
}

#[test]
fn test_edit_parameters_schema() {
    let tool = make_tool();
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("file_path")));
    assert!(required.contains(&json!("old_string")));
    assert!(required.contains(&json!("new_string")));
    assert!(schema["properties"]["file_path"].is_object());
    assert!(schema["properties"]["old_string"].is_object());
    assert!(schema["properties"]["new_string"].is_object());
    assert!(schema["properties"]["replace_all"].is_object());
}
