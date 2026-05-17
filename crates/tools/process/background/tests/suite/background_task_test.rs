use std::time::Duration;

use loopal_tool_api::{PermissionLevel, Tool};
use loopal_tool_background::StatusFilter;
use loopal_tool_background::ops::{bg_output, bg_stop};
use serde_json::json;

use crate::test_support::{extract_pid, make_bash, make_ctx, make_store};

#[test]
fn generate_task_id_is_unique() {
    let store = make_store();
    let id1 = store.generate_task_id();
    let id2 = store.generate_task_id();
    assert_ne!(id1, id2);
    assert!(id1.starts_with("bg_"));
}

#[tokio::test]
async fn bash_background_emits_completed_output() {
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_ctx();

    let result = bash
        .execute(
            json!({"command": "echo bg_hello", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!result.is_error);
    let pid = extract_pid(&result.content);

    let output = bg_output(&store, &pid, true, Duration::from_secs(5)).await;
    assert!(!output.is_error);
    assert!(
        output.content.contains("bg_hello"),
        "expected bg_hello in output: {}",
        output.content,
    );
    assert!(output.content.contains("Completed"));
}

#[tokio::test]
async fn evict_terminal_unlinks_process_log_file() {
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_ctx();

    let result = bash
        .execute(
            json!({"command": "echo evict_probe", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    let pid = extract_pid(&result.content);
    let log_path = extract_log_path(&result.content);

    let _ = bg_output(&store, &pid, true, Duration::from_secs(5)).await;
    assert!(tokio::fs::metadata(&log_path).await.is_ok());

    // reason: evict 0 retention forces immediate removal; unlink is spawned
    // on tokio so we poll briefly for it to land.
    let _ = store.evict_terminal(Duration::from_millis(0));
    for _ in 0..20 {
        if tokio::fs::metadata(&log_path).await.is_err() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "log file should be unlinked after evict: {}",
        log_path.display()
    );
}

fn extract_log_path(content: &str) -> std::path::PathBuf {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("Full log: ") {
            return std::path::PathBuf::from(rest.trim());
        }
    }
    panic!("no Full log line in: {content}")
}

#[tokio::test]
#[cfg(not(windows))]
async fn bash_stop_kills_long_running_process() {
    let store = make_store();
    let bash = make_bash(store.clone());
    let ctx = make_ctx();

    let result = bash
        .execute(
            json!({"command": "sleep 300", "run_in_background": true}),
            &ctx,
        )
        .await
        .unwrap();
    let pid = extract_pid(&result.content);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let stop = bg_stop(&store, &pid).await;
    assert!(
        stop.content.to_lowercase().contains("killed")
            || stop.content.to_lowercase().contains("already"),
        "expected kill ack, got: {} (is_error={})",
        stop.content,
        stop.is_error,
    );

    let still_running = store
        .snapshot(StatusFilter::Running)
        .iter()
        .any(|s| s.id == pid);
    assert!(!still_running, "process should be removed from Running set");
}

#[test]
fn bash_schema_advertises_run_in_background_flag() {
    let store = make_store();
    let tool = make_bash(store);
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["run_in_background"].is_object());
    assert!(schema["properties"]["process_id"].is_null());
    assert_eq!(tool.permission(), PermissionLevel::Dangerous);
}
