use loopal_tool_api::{Tool, PermissionLevel};
use loopal_tool_fetch::FetchTool;

#[test]
fn test_fetch_name() {
    assert_eq!(FetchTool.name(), "Fetch");
}

#[test]
fn test_fetch_permission() {
    assert_eq!(FetchTool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_fetch_schema_requires_url() {
    let schema = FetchTool.parameters_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("url")));
    assert!(schema["properties"]["url"].is_object());
}

#[tokio::test]
async fn test_fetch_missing_url_returns_error() {
    let ctx = loopal_tool_api::ToolContext {
        cwd: std::env::temp_dir(), session_id: "t".into(), shared: None,
    };
    let result = FetchTool.execute(serde_json::json!({}), &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fetch_invalid_url_returns_error() {
    let ctx = loopal_tool_api::ToolContext {
        cwd: std::env::temp_dir(), session_id: "t".into(), shared: None,
    };
    // URL without http(s) scheme is rejected at validation, no network I/O
    let result = FetchTool.execute(serde_json::json!({"url": "not-a-url"}), &ctx).await;
    assert!(result.is_err());
}
