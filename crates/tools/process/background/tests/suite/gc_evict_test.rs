use std::time::Duration;

use loopal_tool_background::StatusFilter;

use crate::test_support::{make_store, spawn_completed_task, spawn_long_running_task};

#[tokio::test]
async fn evict_drops_terminal_tasks_older_than_threshold() {
    let store = make_store();
    let _a = spawn_completed_task(&store, "one").await;
    let _b = spawn_completed_task(&store, "two").await;

    tokio::time::sleep(Duration::from_millis(50)).await;
    let evicted = store.evict_terminal(Duration::from_millis(10));
    assert_eq!(evicted, 2);
    assert_eq!(store.snapshot(StatusFilter::All).len(), 0);
}

#[tokio::test]
async fn evict_preserves_terminal_tasks_within_retention() {
    let store = make_store();
    let _a = spawn_completed_task(&store, "fresh").await;

    let evicted = store.evict_terminal(Duration::from_secs(60));
    assert_eq!(evicted, 0);
    assert_eq!(store.snapshot(StatusFilter::All).len(), 1);
}

#[tokio::test]
async fn evict_does_not_remove_running_tasks() {
    let store = make_store();
    let _ = spawn_long_running_task(&store).await;
    let _ = spawn_long_running_task(&store).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let evicted = store.evict_terminal(Duration::from_millis(10));
    assert_eq!(evicted, 0);
    assert_eq!(store.snapshot(StatusFilter::Running).len(), 2);
}
