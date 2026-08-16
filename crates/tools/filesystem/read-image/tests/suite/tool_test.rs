use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_read_image::{ReadImageParams, ReadImageTool};
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

fn make_tool() -> TypedBridge<ReadImageTool, ReadImageParams> {
    TypedBridge::new(ReadImageTool)
}

fn minimal_png(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    v.extend_from_slice(&[0, 0, 0, 13]);
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    v
}

#[tokio::test]
async fn happy_path_returns_image_block() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.png");
    std::fs::write(&file, minimal_png(40, 50)).unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(result.images.len(), 1);
    assert!(result.content.contains("image/png"));
    assert!(result.content.contains("40×50"));
}

#[tokio::test]
async fn non_image_returns_error_result() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("data.txt");
    std::fs::write(&file, "not an image").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.images.is_empty());
}

#[tokio::test]
async fn missing_file_returns_error_result() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let result = tool
        .execute(
            json!({"file_path": tmp.path().join("nope.png").to_str().unwrap()}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.is_error);
}

#[test]
fn declared_metadata() {
    let tool = TypedBridge::<_, ReadImageParams>::new(ReadImageTool);
    assert_eq!(tool.name(), "ReadImage");
    assert!(tool.description().contains("Supported formats"));
    assert!(matches!(
        tool.permission(),
        loopal_tool_api::PermissionLevel::ReadOnly
    ));
    assert_eq!(tool.secret_eligible_params(), &[] as &[&str]);
    assert_eq!(
        tool.image_output_policy(),
        loopal_tool_api::ImageOutputPolicy::ValidatedInline
    );
}
