use loopal_tool_api::{PermissionLevel, Tool, ToolContext, TypedBridge};
use loopal_tool_fetch::{FetchParams, FetchTool};

#[path = "fetch_behavior_test.rs"]
mod fetch_behavior_test;
#[path = "fetch_failure_backend.rs"]
mod fetch_failure_backend;
#[path = "fetch_failure_test.rs"]
mod fetch_failure_test;
#[path = "fetch_refiner_contract_test.rs"]
mod fetch_refiner_contract_test;
#[path = "fetch_refiner_test.rs"]
mod fetch_refiner_test;

fn make_ctx() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "t")
}

fn make_tool() -> TypedBridge<FetchTool, FetchParams> {
    TypedBridge::new(FetchTool)
}

#[test]
fn test_fetch_name() {
    assert_eq!(make_tool().name(), "Fetch");
}

#[test]
fn test_fetch_description() {
    let tool = make_tool();
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("URL"));
}

#[test]
fn test_fetch_permission() {
    assert_eq!(make_tool().permission(), PermissionLevel::Write);
}

#[test]
fn test_fetch_schema_requires_url() {
    let schema = make_tool().parameters_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("url")));
    assert!(schema["properties"]["url"].is_object());
}

#[tokio::test]
async fn test_fetch_missing_url_returns_error() {
    let ctx = make_ctx();
    let result = make_tool().execute(serde_json::json!({}), &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fetch_invalid_url_returns_error() {
    let ctx = make_ctx();
    let result = make_tool()
        .execute(serde_json::json!({"url": "not-a-url"}), &ctx)
        .await;
    assert!(result.is_err());
}
