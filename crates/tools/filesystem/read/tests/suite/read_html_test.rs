use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_read::ReadTool;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "t")
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
    let result = ReadTool
        .execute(
            serde_json::json!({"file_path": html_path.to_str().unwrap()}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    // html2text converts HTML to plain text — should contain "Hello" and "World"
    assert!(
        result.content.contains("Hello"),
        "content: {}",
        result.content
    );
    assert!(
        result.content.contains("World"),
        "content: {}",
        result.content
    );
    // Should NOT contain HTML tags
    assert!(!result.content.contains("<h1>"));
}

#[tokio::test]
async fn test_read_htm_extension_also_converts() {
    let dir = tempfile::tempdir().unwrap();
    let htm_path = dir.path().join("page.htm");
    std::fs::write(&htm_path, "<html><body><b>Bold</b></body></html>").unwrap();

    let ctx = make_ctx(dir.path());
    let result = ReadTool
        .execute(
            serde_json::json!({"file_path": htm_path.to_str().unwrap()}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("Bold"));
    assert!(!result.content.contains("<b>"));
}

#[tokio::test]
async fn test_read_plain_text_not_affected() {
    let dir = tempfile::tempdir().unwrap();
    let txt_path = dir.path().join("test.txt");
    std::fs::write(&txt_path, "line one\nline two").unwrap();

    let ctx = make_ctx(dir.path());
    let result = ReadTool
        .execute(
            serde_json::json!({"file_path": txt_path.to_str().unwrap()}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    // Should have line numbers (cat -n format)
    assert!(result.content.contains("line one"));
    assert!(result.content.contains("1\t"));
}

#[tokio::test]
async fn test_read_pages_on_non_pdf_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let txt_path = dir.path().join("test.txt");
    std::fs::write(&txt_path, "hello").unwrap();

    let ctx = make_ctx(dir.path());
    let result = ReadTool
        .execute(
            serde_json::json!({"file_path": txt_path.to_str().unwrap(), "pages": "1-3"}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("PDF"));
}
