use loopal_tool_api::{OutputTail, Tool, ToolContext, TypedBridge};
use loopal_tool_bash::{BashParams, BashTool};
use serde_json::json;
use std::sync::Arc;

fn make_tool() -> TypedBridge<BashTool, BashParams> {
    TypedBridge::new(BashTool::new(super::make_store()))
}

fn make_streaming_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test").with_output_tail(Arc::new(OutputTail::new(20)))
}

#[tokio::test]
#[cfg(not(windows))]
async fn streaming_timeout_converts_to_background() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_streaming_ctx(tmp.path());

    let result = tool
        .execute(json!({"command": "sleep 60", "timeout": 0}), &ctx)
        .await
        .unwrap();

    assert!(
        !result.is_error,
        "streaming timeout should be success (bg conversion), got: {}",
        result.content,
    );
    assert!(
        result.content.contains("process_id"),
        "should include background process_id",
    );
}

#[tokio::test]
async fn non_streaming_timeout_is_hard_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let backend = loopal_backend::LocalBackend::new(
        tmp.path().to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    let ctx = ToolContext::new(backend, "test");

    let result = tool
        .execute(json!({"command": "sleep 60", "timeout": 0}), &ctx)
        .await;

    assert!(result.is_err(), "non-streaming timeout should be Err");
}
