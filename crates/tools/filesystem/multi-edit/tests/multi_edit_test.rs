use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_multi_edit::MultiEditTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test")
}

#[test]
fn test_multi_edit_name() {
    let tool = MultiEditTool;
    assert_eq!(tool.name(), "MultiEdit");
}

#[test]
fn test_multi_edit_permission() {
    let tool = MultiEditTool;
    assert_eq!(tool.permission(), PermissionLevel::Supervised);
}

#[test]
fn test_multi_edit_parameters_schema() {
    let tool = MultiEditTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("file_path")));
    assert!(required.contains(&json!("edits")));
}

#[tokio::test]
async fn test_multi_edit_success() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.txt");
    std::fs::write(&file, "hello world\nfoo bar\n").unwrap();

    let tool = MultiEditTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "edits": [
                    { "old_string": "hello", "new_string": "hi" },
                    { "old_string": "foo", "new_string": "baz" }
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error, "unexpected error: {}", result.content);
    assert!(result.content.contains("2 edit(s)"));

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "hi world\nbaz bar\n");
}

#[tokio::test]
async fn test_multi_edit_atomic_rollback() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.txt");
    std::fs::write(&file, "aaa bbb").unwrap();

    let tool = MultiEditTool;
    let ctx = make_ctx(tmp.path());

    // First edit succeeds, second edit fails (not found)
    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "edits": [
                    { "old_string": "aaa", "new_string": "ccc" },
                    { "old_string": "NONEXISTENT", "new_string": "xxx" }
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Edit 1"));
    assert!(result.content.contains("not found"));

    // File must be unchanged (atomic -- nothing written)
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "aaa bbb");
}

#[tokio::test]
async fn test_multi_edit_omission_detection() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.rs");
    std::fs::write(&file, "fn main() {}").unwrap();

    let tool = MultiEditTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "edits": [
                    {
                        "old_string": "fn main() {}",
                        "new_string": "fn main() {\n    // ... existing code\n}"
                    }
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("omission"));
}

#[tokio::test]
async fn test_multi_edit_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.txt");
    std::fs::write(&file, "hello").unwrap();

    let tool = MultiEditTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "edits": [
                    { "old_string": "MISSING", "new_string": "replacement" }
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Edit 0"));
    assert!(result.content.contains("not found"));
}
