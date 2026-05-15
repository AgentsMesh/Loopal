use std::sync::Arc;

use loopal_tool_api::{PermissionLevel, Tool, ToolContext, TypedBridge};
use loopal_tool_background::{BackgroundTask, BackgroundTaskStore, TaskStatus};
use loopal_tool_bash::{BashParams, BashTool};
use serde_json::json;
use std::sync::Mutex;

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

#[test]
fn test_store_insert_and_retrieve() {
    let store = make_store();
    let task_id = store.generate_task_id();
    let (_watch_tx, watch_rx) = tokio::sync::watch::channel(TaskStatus::Running);
    let task = BackgroundTask {
        output: Arc::new(Mutex::new(String::new())),
        exit_code: Arc::new(Mutex::new(None)),
        status: Arc::new(Mutex::new(TaskStatus::Running)),
        description: "test task".into(),
        child: Arc::new(Mutex::new(None)),
        status_watch: watch_rx,
    };
    store.insert(task_id.clone(), task);
    assert!(store.with_task(&task_id, |_| ()).is_some());
}

#[test]
fn test_generate_task_id_is_unique() {
    let store = make_store();
    let id1 = store.generate_task_id();
    let id2 = store.generate_task_id();
    assert_ne!(id1, id2);
    assert!(id1.starts_with("bg_"));
}

#[tokio::test]
async fn test_bash_background_and_output() {
    let tmp = tempfile::tempdir().unwrap();
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_ctx(tmp.path());

    let result = bash
        .execute(
            json!({"command": "echo bg_hello", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("process_id:"));

    let pid = result
        .content
        .lines()
        .find(|l| l.starts_with("process_id:"))
        .and_then(|l| l.strip_prefix("process_id: "))
        .unwrap();

    use loopal_tool_background::ops::bg_output;
    let output = bg_output(&store, pid, true, std::time::Duration::from_secs(5)).await;
    assert!(!output.is_error);
    assert!(
        output.content.contains("bg_hello"),
        "expected bg_hello in output: {}",
        output.content,
    );
    assert!(output.content.contains("Completed"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn test_bash_stop_background() {
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

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    use loopal_tool_background::ops::bg_stop;
    let stop = bg_stop(&store, pid);
    assert!(
        !stop.is_error,
        "bg_stop returned error for {pid}: {}",
        stop.content,
    );
    assert!(
        stop.content.contains("stopped"),
        "unexpected: {}",
        stop.content,
    );
}

#[test]
fn test_bash_schema_includes_background_fields() {
    let store = make_store();
    let tool = make_bash(store);
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["run_in_background"].is_object());
    assert!(schema["properties"]["process_id"].is_null());
    assert_eq!(tool.permission(), PermissionLevel::Dangerous);
}
