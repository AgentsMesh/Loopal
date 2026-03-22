use loopal_tool_api::{Tool, PermissionLevel, ToolContext};
use loopal_tool_fetch::FetchTool;

fn make_ctx() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        backend,
        session_id: "t".into(),
        shared: None,
        pending_cwd_switch: Default::default(), memory_channel: None,
    }
}

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
    let ctx = make_ctx();
    let result = FetchTool.execute(serde_json::json!({}), &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fetch_invalid_url_returns_error() {
    let ctx = make_ctx();
    // URL without http(s) scheme is rejected at validation, no network I/O
    let result = FetchTool.execute(serde_json::json!({"url": "not-a-url"}), &ctx).await;
    assert!(result.is_err());
}
