use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_background::{BackgroundTask, TaskStatus};
use loopal_tool_bash::BashTool;
use serde_json::json;
use std::sync::{Arc, Mutex};

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

#[test]
fn test_store_insert_and_retrieve() {
    let store = loopal_tool_background::store();
    let task_id = loopal_tool_background::generate_task_id();
    let (_watch_tx, watch_rx) = tokio::sync::watch::channel(TaskStatus::Running);
    let task = BackgroundTask {
        output: Arc::new(Mutex::new(String::new())),
        exit_code: Arc::new(Mutex::new(None)),
        status: Arc::new(Mutex::new(TaskStatus::Running)),
        description: "test task".into(),
        child: Arc::new(Mutex::new(None)),
        status_watch: watch_rx,
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
}

/// Bash tool handles background execution and output retrieval.
#[tokio::test]
async fn test_bash_background_and_output() {
    let tmp = tempfile::tempdir().unwrap();
    let bash = BashTool;
    let ctx = make_ctx(tmp.path());

    // Start background process
    let result = bash.execute(
        json!({"command": "echo bg_hello", "run_in_background": true}), &ctx,
    ).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("process_id:"));

    let pid = result.content.lines()
        .find(|l| l.starts_with("process_id:"))
        .and_then(|l| l.strip_prefix("process_id: "))
        .unwrap();

    // Get output via Bash(process_id=...)
    let output = bash.execute(
        json!({"process_id": pid, "block": true, "timeout": 5000}), &ctx,
    ).await.unwrap();
    assert!(!output.is_error);
    assert!(output.content.contains("bg_hello"));
    assert!(output.content.contains("Completed"));
}

/// Bash tool handles stopping background processes.
#[tokio::test]
#[cfg(not(windows))]
async fn test_bash_stop_background() {
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

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Stop via Bash(process_id=..., stop=true)
    let stop = bash.execute(
        json!({"process_id": pid, "stop": true}), &ctx,
    ).await.unwrap();
    assert!(!stop.is_error);
    assert!(stop.content.contains("stopped"));
}

#[test]
fn test_bash_schema_includes_background_fields() {
    let tool = BashTool;
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["run_in_background"].is_object());
    assert!(schema["properties"]["process_id"].is_object());
    assert!(schema["properties"]["stop"].is_object());
    assert_eq!(tool.permission(), PermissionLevel::Dangerous);
}
