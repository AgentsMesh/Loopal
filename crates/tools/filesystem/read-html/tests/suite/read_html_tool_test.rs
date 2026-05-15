use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_read_html::ReadHtmlTool;
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
async fn test_read_html_converts_to_text() {
    let dir = tempfile::tempdir().unwrap();
    let html_path = dir.path().join("test.html");
    std::fs::write(
        &html_path,
        "<html><body><h1>Hello</h1><p>World</p></body></html>",
    )
    .unwrap();

    let ctx = make_ctx(dir.path());
    let result = ReadHtmlTool
        .execute(json!({"file_path": html_path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("Hello"));
    assert!(result.content.contains("World"));
    assert!(!result.content.contains("<h1>"));
}

#[tokio::test]
async fn test_read_htm_extension_also_works() {
    let dir = tempfile::tempdir().unwrap();
    let htm_path = dir.path().join("page.htm");
    std::fs::write(&htm_path, "<html><body><b>Bold</b></body></html>").unwrap();

    let ctx = make_ctx(dir.path());
    let result = ReadHtmlTool
        .execute(json!({"file_path": htm_path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("Bold"));
    assert!(!result.content.contains("<b>"));
}

#[tokio::test]
async fn test_read_html_rejects_non_html() {
    let dir = tempfile::tempdir().unwrap();
    let txt_path = dir.path().join("test.txt");
    std::fs::write(&txt_path, "plain text").unwrap();

    let ctx = make_ctx(dir.path());
    let result = ReadHtmlTool
        .execute(json!({"file_path": txt_path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("only supports .html/.htm"));
}

#[tokio::test]
async fn test_read_html_nonexistent_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.html");

    let ctx = make_ctx(dir.path());
    let result = ReadHtmlTool
        .execute(json!({"file_path": path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
}

#[test]
fn test_read_html_name() {
    assert_eq!(ReadHtmlTool.name(), "ReadHtml");
}

#[test]
fn test_read_html_schema() {
    let schema = ReadHtmlTool.parameters_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("file_path")));
    assert!(schema["properties"]["pages"].is_null());
}
