use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_background::BackgroundTaskStore;
use loopal_tool_bash_process::{BashProcessParams, BashProcessTool};
use serde_json::json;
use std::sync::Arc;

fn make_ctx() -> ToolContext {
    let tmp = std::env::temp_dir();
    let backend = loopal_backend::LocalBackend::new(
        tmp,
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "test")
}

fn make_tool() -> TypedBridge<BashProcessTool, BashProcessParams> {
    let store = BackgroundTaskStore::new();
    TypedBridge::new(BashProcessTool::new(store))
}

fn make_tool_with_store(
    store: Arc<BackgroundTaskStore>,
) -> TypedBridge<BashProcessTool, BashProcessParams> {
    TypedBridge::new(BashProcessTool::new(store))
}

#[test]
fn test_bash_process_name() {
    let tool = make_tool();
    assert_eq!(tool.name(), "BashProcess");
}

#[test]
fn test_bash_process_schema_has_required_process_id() {
    let tool = make_tool();
    let schema = tool.parameters_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("process_id")));
    assert!(!required.contains(&json!("command")));
}

#[test]
fn test_bash_process_schema_has_no_command_field() {
    let tool = make_tool();
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["command"].is_null());
    assert!(schema["properties"]["process_id"].is_object());
    assert!(schema["properties"]["block"].is_object());
    assert!(schema["properties"]["stop"].is_object());
    assert!(schema["properties"]["timeout"].is_object());
}

#[tokio::test]
async fn test_bash_process_not_found() {
    let store = BackgroundTaskStore::new();
    let tool = make_tool_with_store(store);
    let ctx = make_ctx();

    let result = tool
        .execute(json!({"process_id": "bg_999"}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Process not found"));
}

#[tokio::test]
async fn test_bash_process_stop_not_found() {
    let store = BackgroundTaskStore::new();
    let tool = make_tool_with_store(store);
    let ctx = make_ctx();

    let result = tool
        .execute(json!({"process_id": "bg_999", "stop": true}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Process not found"));
}

#[tokio::test]
async fn test_bash_process_missing_process_id_returns_error() {
    let store = BackgroundTaskStore::new();
    let tool = make_tool_with_store(store);
    let ctx = make_ctx();

    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err());
}
