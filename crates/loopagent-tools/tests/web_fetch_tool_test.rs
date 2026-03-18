use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext};
use loopagent_tools::builtin::web_fetch::WebFetchTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    ToolContext {
        cwd: cwd.to_path_buf(),
        session_id: "test".into(),
        shared: None,
    }
}

#[test]
fn test_web_fetch_name() {
    let tool = WebFetchTool;
    assert_eq!(tool.name(), "WebFetch");
}

#[test]
fn test_web_fetch_description() {
    let tool = WebFetchTool;
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("URL"));
}

#[test]
fn test_web_fetch_permission() {
    let tool = WebFetchTool;
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_web_fetch_parameters_schema() {
    let tool = WebFetchTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("url")));
    assert!(schema["properties"]["url"].is_object());
}

#[tokio::test]
async fn test_web_fetch_missing_url_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = WebFetchTool;
    let ctx = make_ctx(tmp.path());

    let result = tool.execute(json!({}), &ctx).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_web_fetch_invalid_url_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = WebFetchTool;
    let ctx = make_ctx(tmp.path());

    // A URL with an unresolvable hostname (`.invalid` TLD is reserved by RFC 2606).
    // Either the DNS lookup fails (Err) or the HTTP response is non-2xx (Ok with is_error).
    let result = tool
        .execute(json!({"url": "http://host.invalid:1/path"}), &ctx)
        .await;

    match result {
        Err(_) => {} // DNS/connection failure — expected on most systems
        Ok(r) => assert!(r.is_error, "expected error result, got: {}", r.content),
    }
}
