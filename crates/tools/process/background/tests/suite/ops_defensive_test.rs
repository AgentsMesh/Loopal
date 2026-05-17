use std::sync::Arc;
use std::time::Duration;

use loopal_tool_api::BgTaskConfig;
use loopal_tool_background::BackgroundTaskStore;
use loopal_tool_background::ops::{bg_output, bg_stop};

use crate::test_support::{
    spawn_completed_task, spawn_failed_task, spawn_long_running_task, spawn_raw,
};

fn store_with_short_ack() -> Arc<BackgroundTaskStore> {
    BackgroundTaskStore::with_config(BgTaskConfig {
        stop_ack_timeout_secs: 1,
        ..BgTaskConfig::default()
    })
}

#[tokio::test]
async fn bg_output_blocking_times_out_for_long_running_task() {
    let store = BackgroundTaskStore::new();
    let pid = spawn_long_running_task(&store).await;

    let result = bg_output(&store, &pid, true, Duration::from_millis(80)).await;
    assert!(!result.is_error);
    assert!(
        result.content.contains("Running (timed out waiting)"),
        "expected timed-out-waiting marker, got: {}",
        result.content
    );
}

#[tokio::test]
async fn bg_output_for_failed_task_shows_failed_status() {
    let store = BackgroundTaskStore::new();
    let pid = spawn_failed_task(&store, "").await;

    let result = bg_output(&store, &pid, false, Duration::from_millis(50)).await;
    assert!(result.is_error);
    assert!(
        result.content.contains("Failed, exit 1") || result.content.contains("[Failed"),
        "expected Failed marker, got: {}",
        result.content
    );
}

#[tokio::test]
#[cfg(not(windows))]
async fn bg_output_for_killed_process_shows_killed_status() {
    let store = BackgroundTaskStore::new();
    let pid = spawn_raw(&store, "sleep 30").await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let stop = bg_stop(&store, &pid).await;
    assert!(!stop.is_error);

    let output = bg_output(&store, &pid, false, Duration::from_millis(50)).await;
    assert!(output.is_error);
    assert!(
        output.content.contains("Killed"),
        "expected Killed marker after stop, got: {}",
        output.content
    );
}

#[tokio::test]
async fn bg_stop_on_already_completed_returns_already_marker() {
    let store = BackgroundTaskStore::new();
    let pid = spawn_completed_task(&store, "").await;
    tokio::time::sleep(Duration::from_millis(20)).await;

    let result = bg_stop(&store, &pid).await;
    assert!(
        !result.is_error,
        "AlreadyTerminal should be reported as success, got error: {}",
        result.content
    );
    assert!(result.content.to_lowercase().contains("already"));
}

#[tokio::test]
async fn bg_stop_with_short_ack_timeout_resolves_for_terminal_task() {
    let store = store_with_short_ack();
    let pid = spawn_completed_task(&store, "").await;
    tokio::time::sleep(Duration::from_millis(20)).await;

    let result = bg_stop(&store, &pid).await;
    assert!(!result.is_error);
}
