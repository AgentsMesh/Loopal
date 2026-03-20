use loopal_tool_api::{Tool, ToolContext};
use loopal_tools::builtin::read::ReadTool;
use loopal_tools::builtin::read_pdf::parse_page_range;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    ToolContext {
        cwd: cwd.to_path_buf(),
        session_id: "test".into(),
        shared: None,
    }
}

// --- parse_page_range tests ---

#[test]
fn test_parse_single_page() {
    let result = parse_page_range("3", 10).unwrap();
    assert_eq!(result, vec![2]); // 0-based
}

#[test]
fn test_parse_page_range_inclusive() {
    let result = parse_page_range("2-5", 10).unwrap();
    assert_eq!(result, vec![1, 2, 3, 4]); // 0-based
}

#[test]
fn test_parse_range_clamped_to_total() {
    let result = parse_page_range("3-20", 5).unwrap();
    assert_eq!(result, vec![2, 3, 4]); // pages 3,4,5 (0-based)
}

#[test]
fn test_parse_first_page() {
    let result = parse_page_range("1", 1).unwrap();
    assert_eq!(result, vec![0]);
}

#[test]
fn test_parse_page_zero_error() {
    let result = parse_page_range("0", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("1-based"));
}

#[test]
fn test_parse_range_zero_start_error() {
    let result = parse_page_range("0-5", 10);
    assert!(result.is_err());
}

#[test]
fn test_parse_page_exceeds_total() {
    let result = parse_page_range("11", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceeds total"));
}

#[test]
fn test_parse_range_start_exceeds_total() {
    let result = parse_page_range("11-15", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceeds total"));
}

#[test]
fn test_parse_inverted_range_error() {
    let result = parse_page_range("5-3", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("start"));
}

#[test]
fn test_parse_empty_spec_error() {
    let result = parse_page_range("", 10);
    assert!(result.is_err());
}

#[test]
fn test_parse_malformed_spec_error() {
    let result = parse_page_range("abc", 10);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid page number"));
}

// --- extract_pdf_text with non-existent file ---

#[test]
fn test_extract_nonexistent_file_error() {
    let result =
        loopal_tools::builtin::read_pdf::extract_pdf_text(std::path::Path::new("/no/such.pdf"), None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to extract"));
}

// --- ReadTool with pages param on non-PDF ---

#[tokio::test]
async fn test_read_pages_on_non_pdf_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("data.txt");
    std::fs::write(&file, "hello").unwrap();

    let tool = ReadTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap(),
                "pages": "1-3"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("only supported for PDF"));
}

#[tokio::test]
async fn test_read_pdf_extension_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("test.pdf");
    // Write invalid PDF content - should produce an extraction error, not a panic
    std::fs::write(&file, "not a real pdf").unwrap();

    let tool = ReadTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "file_path": file.to_str().unwrap()
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Should try PDF extraction and return an error (not a panic)
    assert!(result.is_error);
    assert!(result.content.contains("Failed to extract"));
}

#[test]
fn test_read_schema_includes_pages() {
    let tool = ReadTool;
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["pages"].is_object());
    let desc = schema["properties"]["pages"]["description"].as_str().unwrap();
    assert!(desc.contains("PDF"));
}
