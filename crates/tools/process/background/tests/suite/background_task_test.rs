use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_background::task_output::TaskOutputTool;
use loopal_tool_background::task_stop::TaskStopTool;
use loopal_tool_background::{BackgroundTask, TaskStatus};
use loopal_tool_bash::BashTool;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        session_id: "test".into(),
        shared: None,
        pending_cwd_switch: Default::default(),
        memory_channel: None,
        output_tail: None,
        backend,
    }
}

#[test]
fn test_store_insert_and_retrieve() {
    let store = loopal_tool_background::store();
    let task_id = loopal_tool_background::generate_task_id();

    let task = BackgroundTask {
        output: Arc::new(Mutex::new(String::new())),
        exit_code: Arc::new(Mutex::new(None)),
        status: Arc::new(Mutex::new(TaskStatus::Running)),
        description: "test task".into(),
        child: Arc::new(Mutex::new(None)),
    };

    store.lock().unwrap().insert(task_id.clone(), task);
    assert!(store.lock().unwrap().contains_key(&task_id));
    store.lock().unwrap().remove(&task_id);
}

#[test]
fn test_generate_task_id_is_unique() {
    let id1 = loopal_tool_background::generate_task_id();
    let id2 = loopal_tool_background::generate_task_id();
    assert_ne!(id1, id2);
    assert!(id1.starts_with("bg_"));
    assert!(id2.starts_with("bg_"));
}

#[test]
fn test_task_output_tool_metadata() {
    let tool = TaskOutputTool;
    assert_eq!(tool.name(), "TaskOutput");
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
    let schema = tool.parameters_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("task_id")));
}

#[test]
fn test_task_stop_tool_metadata() {
    let tool = TaskStopTool;
    assert_eq!(tool.name(), "TaskStop");
    assert_eq!(tool.permission(), PermissionLevel::Supervised);
    let schema = tool.parameters_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("task_id")));
}

#[tokio::test]
async fn test_background_execution_and_poll_output() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = BashTool;
    let ctx = make_ctx(tmp.path());

    let result = bash
        .execute(
            json!({
                "command": "echo bg_hello",
                "run_in_background": true,
                "description": "echo test"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.starts_with("Background task started: bg_"));

    let task_id = result
        .content
        .strip_prefix("Background task started: ")
        .unwrap();

    let output_tool = TaskOutputTool;
    let output = output_tool
        .execute(
            json!({ "task_id": task_id, "block": true, "timeout": 5000 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!output.is_error);
    assert!(output.content.contains("bg_hello"));
    assert!(output.content.contains("[Status: Completed"));
}

#[tokio::test]
#[cfg(not(windows))] // cmd.exe child processes become orphans on kill, blocking pipe reads
async fn test_stop_running_task() {
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

    // Small delay to ensure the process is actually running
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let stop_tool = TaskStopTool;
    let stop_result = stop_tool
        .execute(json!({ "task_id": task_id }), &ctx)
        .await
        .unwrap();

    assert!(!stop_result.is_error);
    assert!(stop_result.content.contains("Task stopped"));
}

#[test]
fn test_bash_schema_includes_background_fields() {
    let tool = BashTool;
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["run_in_background"].is_object());
    assert!(schema["properties"]["description"].is_object());
}
