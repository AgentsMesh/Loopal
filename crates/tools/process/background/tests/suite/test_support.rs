use std::sync::Arc;
use std::time::Duration;

use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_background::{BackgroundTaskStore, ops::bg_output};
use loopal_tool_bash::{BashParams, BashTool};
use serde_json::json;

pub fn make_store() -> Arc<BackgroundTaskStore> {
    BackgroundTaskStore::new()
}

pub fn make_bash(store: Arc<BackgroundTaskStore>) -> TypedBridge<BashTool, BashParams> {
    TypedBridge::new(BashTool::new(store))
}

/// Build a per-test unique session id. A literal like "test-session" would
/// race against parallel runs (`bazel test --runs_per_test=N`) that share
/// `$TMPDIR/loopal/`, where one process's cleanup can wipe another's log dir.
pub fn unique_sid() -> String {
    format!("test-{}", uuid::Uuid::new_v4().simple())
}

pub fn make_ctx() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        unique_sid(),
    );
    ToolContext::new(backend, "test")
}

pub fn extract_pid(content: &str) -> String {
    content
        .lines()
        .find_map(|l| l.strip_prefix("process_id: "))
        .expect("process_id line missing")
        .to_string()
}

async fn bash_spawn(store: &Arc<BackgroundTaskStore>, cmd: &str) -> String {
    let result = make_bash(store.clone())
        .execute(
            json!({"command": cmd, "run_in_background": true}),
            &make_ctx(),
        )
        .await
        .expect("bash spawn");
    extract_pid(&result.content)
}

pub async fn spawn_completed_task(store: &Arc<BackgroundTaskStore>, output: &str) -> String {
    let cmd = if output.is_empty() {
        "true".to_string()
    } else {
        format!("printf %s {}", shell_quote(output))
    };
    let pid = bash_spawn(store, &cmd).await;
    let _ = bg_output(store, &pid, true, Duration::from_secs(5)).await;
    pid
}

pub async fn spawn_failed_task(store: &Arc<BackgroundTaskStore>, output: &str) -> String {
    let cmd = if output.is_empty() {
        "exit 1".to_string()
    } else {
        format!("printf %s {} ; exit 1", shell_quote(output))
    };
    let pid = bash_spawn(store, &cmd).await;
    let _ = bg_output(store, &pid, true, Duration::from_secs(5)).await;
    pid
}

pub async fn spawn_long_running_task(store: &Arc<BackgroundTaskStore>) -> String {
    let pid = bash_spawn(store, "sleep 30").await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    pid
}

pub async fn spawn_raw(store: &Arc<BackgroundTaskStore>, cmd: &str) -> String {
    bash_spawn(store, cmd).await
}

fn shell_quote(s: &str) -> String {
    let escaped = s.replace('\'', "'\\''");
    format!("'{escaped}'")
}
