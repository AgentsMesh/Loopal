use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_read_pdf::{ReadPdfParams, ReadPdfTool};
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

fn make_tool() -> TypedBridge<ReadPdfTool, ReadPdfParams> {
    TypedBridge::new(ReadPdfTool)
}

#[tokio::test]
async fn test_read_pdf_rejects_non_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("data.txt");
    std::fs::write(&file, "hello").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("only supports .pdf"));
}

#[tokio::test]
async fn test_read_pdf_invalid_content() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.pdf");
    std::fs::write(&file, "not a real pdf").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Failed to extract"));
}

#[tokio::test]
async fn test_read_pdf_nonexistent_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("missing.pdf");

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
}

#[tokio::test]
async fn test_read_pdf_empty_pages_treated_as_none() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.pdf");
    std::fs::write(&file, "not a real pdf").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({"file_path": file.to_str().unwrap(), "pages": ""}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Failed to extract"));
}

#[test]
fn test_read_pdf_name() {
    let tool = make_tool();
    assert_eq!(tool.name(), "ReadPdf");
}

#[test]
fn test_read_pdf_schema_has_pages() {
    let tool = make_tool();
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["pages"].is_object());
}
