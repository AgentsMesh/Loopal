use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_read::{ReadParams, ReadTool};
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
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

#[tokio::test]
async fn test_read_binary_file_rejected_with_actionable_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("data.bin");
    let mut payload = vec![0xFFu8, 0xFE, 0x00, 0x42, 0x00, 0x99];
    payload.extend(std::iter::repeat_n(0x88, 32));
    std::fs::write(&file, payload).unwrap();

    let tool = TypedBridge::<ReadTool, ReadParams>::new(ReadTool);
    let backend = loopal_backend::LocalBackend::new(
        tmp.path().to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    let ctx = ToolContext::new(backend, "test");

    let r = tool
        .execute(json!({"file_path": "data.bin"}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error, "binary file must be rejected: {}", r.content);
    assert!(
        r.content.contains("binary"),
        "error should mention 'binary' for LLM context, got: {}",
        r.content
    );
}

#[tokio::test]
async fn test_read_over_size_limit_hints_to_use_offset_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("big.txt");
    std::fs::write(&file, "x".repeat(500)).unwrap();

    let limits = loopal_backend::ResourceLimits {
        max_file_read_bytes: 100,
        ..loopal_backend::ResourceLimits::default()
    };
    let backend =
        loopal_backend::LocalBackend::new(tmp.path().to_path_buf(), None, limits, "test-session");
    let ctx = ToolContext::new(backend, "test");
    let tool = TypedBridge::<ReadTool, ReadParams>::new(ReadTool);

    let r = tool
        .execute(json!({"file_path": "big.txt"}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(
        r.content.contains("too large"),
        "expected size-limit phrase, got: {}",
        r.content
    );
    assert!(
        r.content.contains("offset") && r.content.contains("limit"),
        "TooLarge error must hint at offset/limit pagination for LLM, got: {}",
        r.content
    );
}
