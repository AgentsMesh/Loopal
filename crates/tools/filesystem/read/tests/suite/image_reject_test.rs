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
async fn rejects_png_with_readimage_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.png");
    std::fs::write(&file, b"\x89PNG\r\n\x1a\nrestofpng").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("ReadImage"));
}

#[tokio::test]
async fn rejects_jpeg_with_readimage_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.jpg");
    std::fs::write(&file, b"\xff\xd8\xffrest").unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("ReadImage"));
}

#[tokio::test]
async fn rejects_gif_with_readimage_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.gif");
    std::fs::write(&file, b"GIF89a\x00\x00\x00\x00").unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("ReadImage"));
}

#[tokio::test]
async fn rejects_webp_with_readimage_hint() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.webp");
    std::fs::write(&file, b"RIFF\x00\x00\x00\x00WEBPrest").unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("ReadImage"));
}

#[tokio::test]
async fn allows_plain_text_files_normally() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.txt");
    std::fs::write(&file, "hello world").unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("hello world"));
}
