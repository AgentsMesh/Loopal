use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_background::task_output::TaskOutputTool;
use loopal_tool_background::task_stop::TaskStopTool;
use loopal_tool_bash::BashTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        session_id: "test".into(),
        shared: None,
        pending_cwd_switch: Default::default(), memory_channel: None,
        backend,
    }
}

#[tokio::test]
async fn test_task_output_nonexistent_task() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = TaskOutputTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({ "task_id": "bg_nonexistent_99999" }), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Task not found"));
}

#[tokio::test]
async fn test_task_stop_nonexistent_task() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = TaskStopTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({ "task_id": "bg_nonexistent_99999" }), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("Task not found"));
}

#[tokio::test]
async fn test_task_output_non_blocking() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = BashTool;
    let ctx = make_ctx(tmp.path());

    let result = bash
        .execute(
            json!({
                "command": "sleep 300",
                "run_in_background": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let task_id = result
        .content
        .strip_prefix("Background task started: ")
        .unwrap();

    let output_tool = TaskOutputTool;
    let output = output_tool
        .execute(
            json!({ "task_id": task_id, "block": false }),
            &ctx,
        )
        .await
        .unwrap();

    // Non-blocking should return immediately with Running status
    assert!(!output.is_error);
    assert!(output.content.contains("[Status: Running]"));

    // Clean up: stop the long-running task
    let stop_tool = TaskStopTool;
    let _ = stop_tool
        .execute(json!({ "task_id": task_id }), &ctx)
        .await;
}

#[tokio::test]
async fn test_task_output_timeout_while_running() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = BashTool;
    let ctx = make_ctx(tmp.path());

    let result = bash
        .execute(
            json!({
                "command": "sleep 300",
                "run_in_background": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let task_id = result
        .content
        .strip_prefix("Background task started: ")
        .unwrap();

    let output_tool = TaskOutputTool;
    let output = output_tool
        .execute(
            json!({ "task_id": task_id, "block": true, "timeout": 200 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!output.is_error);
    assert!(output.content.contains("timed out waiting"));

    // Clean up
    let stop_tool = TaskStopTool;
    let _ = stop_tool
        .execute(json!({ "task_id": task_id }), &ctx)
        .await;
}
