use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_read_pdf::ReadPdfTool;
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
async fn test_read_pdf_rejects_non_pdf() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("data.txt");
    std::fs::write(&file, "hello").unwrap();

    let tool = ReadPdfTool;
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

    let tool = ReadPdfTool;
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

    let tool = ReadPdfTool;
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

    let tool = ReadPdfTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"file_path": file.to_str().unwrap(), "pages": ""}), &ctx)
        .await
        .unwrap();

    // Should attempt extraction (not fail on empty pages param)
    assert!(result.is_error);
    assert!(result.content.contains("Failed to extract"));
}

#[test]
fn test_read_pdf_name() {
    assert_eq!(ReadPdfTool.name(), "ReadPdf");
}

#[test]
fn test_read_pdf_schema_has_pages() {
    let schema = ReadPdfTool.parameters_schema();
    assert!(schema["properties"]["pages"].is_object());
}
