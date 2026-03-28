use loopal_tool_api::{Tool, ToolContext};
#[cfg(not(windows))]
use loopal_tool_bash::BashTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(), None, loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        session_id: "test".into(),
        shared: None,
        memory_channel: None,
        output_tail: None,
        backend,
    }
}

/// Bash(process_id=nonexistent) returns error.
#[tokio::test]
async fn test_output_nonexistent_process() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = loopal_tool_bash::BashTool;
    let ctx = make_ctx(tmp.path());

    let result = bash.execute(
        json!({"process_id": "bg_nonexistent_99999"}), &ctx,
    ).await.unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}

/// Bash(process_id=nonexistent, stop=true) returns error.
#[tokio::test]
async fn test_stop_nonexistent_process() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = loopal_tool_bash::BashTool;
    let ctx = make_ctx(tmp.path());

    let result = bash.execute(
        json!({"process_id": "bg_nonexistent_99999", "stop": true}), &ctx,
    ).await.unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}

/// Non-blocking output returns Running immediately.
#[tokio::test]
#[cfg(not(windows))]
async fn test_non_blocking_output() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = BashTool;
    let ctx = make_ctx(tmp.path());

    let result = bash.execute(
        json!({"command": "sleep 300", "run_in_background": true}), &ctx,
    ).await.unwrap();
    let pid = result.content.lines()
        .find(|l| l.starts_with("process_id:"))
        .and_then(|l| l.strip_prefix("process_id: "))
        .unwrap();

    let output = bash.execute(
        json!({"process_id": pid, "block": false}), &ctx,
    ).await.unwrap();
    assert!(output.content.contains("[Status: Running]"));

    // Cleanup
    let _ = bash.execute(json!({"process_id": pid, "stop": true}), &ctx).await;
}

/// Blocking with short timeout returns timed-out status.
#[tokio::test]
#[cfg(not(windows))]
async fn test_output_timeout() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = BashTool;
    let ctx = make_ctx(tmp.path());

    let result = bash.execute(
        json!({"command": "sleep 300", "run_in_background": true}), &ctx,
    ).await.unwrap();
    let pid = result.content.lines()
        .find(|l| l.starts_with("process_id:"))
        .and_then(|l| l.strip_prefix("process_id: "))
        .unwrap();

    let output = bash.execute(
        json!({"process_id": pid, "block": true, "timeout": 200}), &ctx,
    ).await.unwrap();
    assert!(output.content.contains("timed out"));

    let _ = bash.execute(json!({"process_id": pid, "stop": true}), &ctx).await;
}
