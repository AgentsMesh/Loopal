use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_read::{ReadParams, ReadTool};
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test")
}

fn make_tool() -> TypedBridge<ReadTool, ReadParams> {
    TypedBridge::new(ReadTool)
}

#[tokio::test]
async fn test_read_with_relative_path() {
    let tmp = tempfile::tempdir().unwrap();
    let canon = tmp.path().canonicalize().unwrap();
    std::fs::write(canon.join("rel.txt"), "relative content").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(&canon);

    let result = tool
        .execute(json!({"file_path": "rel.txt"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("relative content"));
}

#[tokio::test]
async fn test_read_output_has_line_numbers() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("numbered.txt");
    std::fs::write(&file, "alpha\nbeta\ngamma").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("1\talpha"));
    assert!(result.content.contains("2\tbeta"));
    assert!(result.content.contains("3\tgamma"));
}

#[tokio::test]
async fn test_read_offset_beyond_file_length() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("short.txt");
    std::fs::write(&file, "only one line").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({"file_path": file.to_str().unwrap(), "offset": 100}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.is_empty() || result.content.trim().is_empty());
}

#[tokio::test]
async fn test_read_path_traversal_protection() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("secret.txt");
    std::fs::write(&outside, "secret data").unwrap();

    let cwd = tmp.path().join("inner");
    std::fs::create_dir_all(&cwd).unwrap();

    let tool = make_tool();
    let ctx = make_ctx(&cwd);

    let result = tool
        .execute(json!({"file_path": "../secret.txt"}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("path escapes working directory"));
}

#[tokio::test]
async fn test_read_empty_string_optional_fields_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.txt");
    std::fs::write(&file, "content here").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({"file_path": file.to_str().unwrap(), "offset": null, "limit": null}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("content here"));
}
