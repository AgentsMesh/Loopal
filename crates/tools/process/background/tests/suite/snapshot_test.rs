use loopal_protocol::BgTaskStatus;
use loopal_tool_background::StatusFilter;

use crate::test_support::{
    make_store, spawn_completed_task, spawn_failed_task, spawn_long_running_task,
};

#[test]
fn empty_store_returns_empty_vec() {
    let store = make_store();
    assert!(store.snapshot(StatusFilter::Running).is_empty());
    assert!(store.snapshot(StatusFilter::All).is_empty());
}

#[tokio::test]
async fn running_filter_excludes_completed_task() {
    let store = make_store();
    let p_run = spawn_long_running_task(&store).await;
    let _p_done = spawn_completed_task(&store, "").await;

    let snaps = store.snapshot(StatusFilter::Running);
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].id, p_run);
    assert_eq!(snaps[0].status, BgTaskStatus::Running);
}

#[tokio::test]
async fn snapshot_sorted_by_id() {
    let store = make_store();
    let mut spawned = Vec::new();
    for _ in 0..3 {
        spawned.push(spawn_long_running_task(&store).await);
    }
    let snaps = store.snapshot(StatusFilter::Running);
    let mut expected = spawned.clone();
    expected.sort();
    let ids: Vec<&str> = snaps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, expected);
}

#[tokio::test]
async fn running_filter_excludes_failed_task() {
    let store = make_store();
    let p_ok = spawn_long_running_task(&store).await;
    let _p_fail = spawn_failed_task(&store, "").await;

    let snaps = store.snapshot(StatusFilter::Running);
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].id, p_ok);
}

#[tokio::test]
async fn terminal_filter_returns_completed_and_failed() {
    let store = make_store();
    let _running = spawn_long_running_task(&store).await;
    let done = spawn_completed_task(&store, "").await;
    let fail = spawn_failed_task(&store, "").await;

    let snaps = store.snapshot(StatusFilter::Terminal);
    let ids: Vec<&str> = snaps.iter().map(|s| s.id.as_str()).collect();
    let mut expected = vec![done.as_str(), fail.as_str()];
    expected.sort();
    assert_eq!(ids, expected);
}

#[tokio::test]
async fn snapshot_carries_description() {
    let store = make_store();
    let _pid = spawn_long_running_task(&store).await;
    let snaps = store.snapshot(StatusFilter::Running);
    assert!(snaps[0].description.contains("sleep 30"));
}
