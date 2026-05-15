use std::sync::Arc;

use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_background::BackgroundTaskStore;
use loopal_tool_bash::{BashParams, BashTool};
use serde_json::json;

fn make_store() -> Arc<BackgroundTaskStore> {
    BackgroundTaskStore::new()
}

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test")
}

fn make_bash(store: Arc<BackgroundTaskStore>) -> TypedBridge<BashTool, BashParams> {
    TypedBridge::new(BashTool::new(store))
}

#[tokio::test]
async fn test_output_nonexistent_process() {
    let store = make_store();
    let timeout = std::time::Duration::from_secs(1);
    let output =
        loopal_tool_background::ops::bg_output(&store, "bg_nonexistent_99999", true, timeout).await;
    assert!(output.is_error);
    assert!(output.content.contains("not found"));
}

#[tokio::test]
async fn test_stop_nonexistent_process() {
    let store = make_store();
    let result = loopal_tool_background::ops::bg_stop(&store, "bg_nonexistent_99999");
    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn test_non_blocking_output() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_ctx(tmp.path());

    let result = bash
        .execute(
            json!({"command": "sleep 300", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    let pid = result
        .content
        .lines()
        .find(|l| l.starts_with("process_id:"))
        .and_then(|l| l.strip_prefix("process_id: "))
        .unwrap();

    let timeout = std::time::Duration::from_secs(1);
    let output = loopal_tool_background::ops::bg_output(&store, pid, false, timeout).await;
    assert!(output.content.contains("[Status: Running]"));

    loopal_tool_background::ops::bg_stop(&store, pid);
}

#[tokio::test]
#[cfg(not(windows))]
async fn test_output_timeout() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_ctx(tmp.path());

    let result = bash
        .execute(
            json!({"command": "sleep 300", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    let pid = result
        .content
        .lines()
        .find(|l| l.starts_with("process_id:"))
        .and_then(|l| l.strip_prefix("process_id: "))
        .unwrap();

    let timeout = std::time::Duration::from_secs(1);
    let output = loopal_tool_background::ops::bg_output(&store, pid, true, timeout).await;
    assert!(output.content.contains("timed out"));

    loopal_tool_background::ops::bg_stop(&store, pid);
}
