use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_web_search::WebSearchTool;
use serde_json::json;

fn make_ctx() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::path::PathBuf::from("/tmp"),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        backend,
        session_id: "test".into(),
        shared: None,
        pending_cwd_switch: Default::default(),
    }
}

#[test]
fn test_web_search_name() {
    let tool = WebSearchTool;
    assert_eq!(tool.name(), "WebSearch");
}

#[test]
fn test_web_search_description() {
    let tool = WebSearchTool;
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Tavily"));
}

#[test]
fn test_web_search_permission() {
    let tool = WebSearchTool;
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_web_search_parameters_schema() {
    let tool = WebSearchTool;
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
    let tool = WebSearchTool;
    let ctx = make_ctx();

    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_web_search_missing_api_key_returns_error() {
    // Temporarily remove the env var to guarantee it's not set during this test.
    // We save and restore in case it exists.
    let saved = std::env::var("TAVILY_API_KEY").ok();
    // SAFETY: test is single-threaded for this env manipulation
    unsafe { std::env::remove_var("TAVILY_API_KEY") };

    let tool = WebSearchTool;
    let ctx = make_ctx();

    let result = tool.execute(json!({"query": "rust lang"}), &ctx).await;

    // Restore
    if let Some(val) = saved {
        // SAFETY: restoring the original env value
        unsafe { std::env::set_var("TAVILY_API_KEY", val) };
    }

    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("TAVILY_API_KEY"));
}
