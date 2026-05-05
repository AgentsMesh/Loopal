/// Tests for streaming exec timeout → ExecOutcome::TimedOut → background conversion.
use loopal_tool_api::{OutputTail, Tool, ToolContext};
use loopal_tool_bash::BashTool;
use serde_json::json;
use std::sync::Arc;

fn make_streaming_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test").with_output_tail(Arc::new(OutputTail::new(20)))
}

/// Streaming timeout produces a success result (converted to background),
/// NOT a Timeout error.
#[tokio::test]
#[cfg(not(windows))]
async fn streaming_timeout_converts_to_background() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = BashTool::new(super::make_store());
    let ctx = make_streaming_ctx(tmp.path());

    let result = bash
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

    let pid = result
        .content
        .lines()
        .find(|l| l.starts_with("process_id:"))
        .and_then(|l| l.strip_prefix("process_id: "))
        .unwrap();

    let output = bash
        .execute(json!({"process_id": pid, "block": false}), &ctx)
        .await
        .unwrap();
    assert!(
        output.content.contains("Running"),
        "bg task should be running",
    );

    let _ = bash
        .execute(json!({"process_id": pid, "stop": true}), &ctx)
        .await;
}

/// Non-streaming timeout (no output_tail) still produces a hard Timeout error.
#[tokio::test]
async fn non_streaming_timeout_is_hard_error() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = BashTool::new(super::make_store());
    let backend = loopal_backend::LocalBackend::new(
        tmp.path().to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    let ctx = ToolContext::new(backend, "test");

    let result = bash
        .execute(json!({"command": "sleep 60", "timeout": 0}), &ctx)
        .await;

    assert!(result.is_err(), "non-streaming timeout should be Err");
}
