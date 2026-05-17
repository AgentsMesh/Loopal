use std::sync::Arc;
use std::time::Duration;

use loopal_tool_api::OutputTail;
use loopal_tool_background::ops::bg_output;

use crate::test_support::{make_store, spawn_completed_task};

#[tokio::test]
async fn completed_task_render_preview_includes_output_line() {
    let store = make_store();
    let pid = spawn_completed_task(&store, "captured-line").await;

    let result = bg_output(&store, &pid, false, Duration::from_millis(100)).await;
    assert!(!result.is_error);
    assert!(result.content.contains("captured-line"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn process_render_preview_contains_log_path_and_stdout_section() {
    use loopal_tool_api::{Tool, ToolContext, TypedBridge};
    use loopal_tool_bash::{BashParams, BashTool};
    use serde_json::json;

    let tmp = tempfile::tempdir().unwrap();
    let store = make_store();
    let bash = TypedBridge::new(BashTool::new(store.clone()));
    let backend = loopal_backend::LocalBackend::new(
        tmp.path().to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    let ctx = ToolContext::new(backend, "test").with_output_tail(Arc::new(OutputTail::new(20)));
    let bridge: TypedBridge<BashTool, BashParams> = bash;

    let result = bridge
        .execute(
            json!({"command": "echo PROCESS_PROBE", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    let pid = result
        .content
        .lines()
        .find(|l| l.starts_with("process_id:"))
        .and_then(|l| l.strip_prefix("process_id: "))
        .unwrap()
        .to_string();

    let output = bg_output(&store, &pid, true, Duration::from_secs(3)).await;
    assert!(output.content.contains("PROCESS_PROBE"));
    assert!(output.content.contains("[stdout"));
    assert!(output.content.contains("[full log:"));
    assert!(output.content.contains("loopal/"));
}

#[tokio::test]
async fn render_preview_for_unknown_task_returns_error() {
    let store = make_store();
    let result = bg_output(&store, "bg_nope", false, Duration::from_millis(50)).await;
    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}
