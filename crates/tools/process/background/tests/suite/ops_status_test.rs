use std::sync::Arc;
use std::time::Duration;

use loopal_tool_background::ops::{bg_output, bg_stop};
use loopal_tool_background::{BackgroundTaskStore, TaskStatus};

use crate::test_support::{spawn_completed_task, spawn_failed_task, spawn_long_running_task};

#[tokio::test]
#[cfg(not(windows))]
async fn stop_on_terminal_task_returns_already_terminal_message() {
    let store = BackgroundTaskStore::new();
    let pid = spawn_completed_task(&store, "").await;

    let result = bg_stop(&store, &pid).await;
    assert!(!result.is_error);
    assert!(
        result.content.contains("already") || result.content.contains("Already"),
        "expected already-terminal message, got: {}",
        result.content
    );
}

#[tokio::test]
#[cfg(not(windows))]
async fn output_for_completed_task_shows_completed_status() {
    let store = BackgroundTaskStore::new();
    let pid = spawn_completed_task(&store, "captured").await;

    let result = bg_output(&store, &pid, false, Duration::from_millis(100)).await;
    assert!(!result.is_error);
    assert!(result.content.contains("captured"));
    assert!(result.content.contains("Completed") || result.content.contains("[Completed"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn output_for_failed_task_shows_failed_status() {
    let store = BackgroundTaskStore::new();
    let pid = spawn_failed_task(&store, "bad").await;

    let result = bg_output(&store, &pid, false, Duration::from_millis(100)).await;
    assert!(result.is_error);
    assert!(result.content.contains("bad"));
    assert!(result.content.contains("Failed") || result.content.contains("[Failed"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn output_blocks_until_status_changes_to_terminal() {
    let store = BackgroundTaskStore::new();
    // reason: spawn a script that holds the task in Running for ~300ms before
    // exiting — gives the assertion below time to observe Running, then the
    // waiter's bg_output(block=true) completes naturally.
    let pid = {
        use crate::test_support::spawn_raw;
        spawn_raw(&store, "sleep 0.3 ; printf %s delayed").await
    };

    let store_clone: Arc<BackgroundTaskStore> = store.clone();
    let pid_clone = pid.clone();
    let waiter = tokio::spawn(async move {
        bg_output(&store_clone, &pid_clone, true, Duration::from_secs(5)).await
    });
    tokio::time::sleep(Duration::from_millis(80)).await;

    let status = store.read_task(&pid, |t| t.status()).unwrap();
    assert_eq!(status, TaskStatus::Running);

    let result = waiter.await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("delayed"));

    // hint to silence the unused helper import in case it became dead
    let _ = spawn_long_running_task;
}
