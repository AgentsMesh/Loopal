use loopal_tool_api::{PermissionLevel, Tool, ToolContext, TypedBridge};
use loopal_tool_web_search::{WebSearchParams, WebSearchTool};
use serde_json::json;

fn make_ctx() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::path::PathBuf::from("/tmp"),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "test")
}

fn make_tool() -> impl Tool {
    TypedBridge::<WebSearchTool, WebSearchParams>::new(WebSearchTool)
}

#[test]
fn test_web_search_name() {
    let tool = make_tool();
    assert_eq!(tool.name(), "WebSearch");
}

#[test]
fn test_web_search_description() {
    let tool = make_tool();
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Search the web"));
}

#[test]
fn test_web_search_permission() {
    let tool = make_tool();
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_web_search_parameters_schema() {
    let tool = make_tool();
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("query")));
    assert!(!required.contains(&json!("allowed_domains")));

    assert!(schema["properties"]["query"].is_object());
    assert!(schema["properties"]["allowed_domains"].is_object());
    assert!(schema["properties"]["blocked_domains"].is_object());
}

#[tokio::test]
async fn test_web_search_missing_query_returns_error() {
    let tool = make_tool();
    let ctx = make_ctx();

    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_web_search_missing_api_key_returns_error() {
    let saved = std::env::var("TAVILY_API_KEY").ok();
    unsafe { std::env::remove_var("TAVILY_API_KEY") };

    let tool = make_tool();
    let ctx = make_ctx();

    let result = tool.execute(json!({"query": "rust lang"}), &ctx).await;

    if let Some(val) = saved {
        unsafe { std::env::set_var("TAVILY_API_KEY", val) };
    }

    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("TAVILY_API_KEY"));
}
