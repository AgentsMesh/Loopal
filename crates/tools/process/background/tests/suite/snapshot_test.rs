/// Tests for snapshot_running() — protocol-level snapshot generation.
use std::sync::Arc;

use loopal_protocol::BgTaskStatus;
use loopal_tool_background::BackgroundTaskStore;

fn make_store() -> Arc<BackgroundTaskStore> {
    BackgroundTaskStore::new()
}

#[test]
fn empty_store_returns_empty_vec() {
    let store = make_store();
    assert!(store.snapshot_running().is_empty());
}

#[test]
fn returns_only_running_tasks() {
    let store = make_store();
    store.register_proxy("bg_1".into(), "running task".into());
    let handle = store.register_proxy("bg_2".into(), "done task".into());
    handle.complete("output".into(), true);

    let snaps = store.snapshot_running();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].id, "bg_1");
    assert_eq!(snaps[0].status, BgTaskStatus::Running);
}

#[test]
fn sorted_by_id() {
    let store = make_store();
    store.register_proxy("bg_c".into(), "c".into());
    store.register_proxy("bg_a".into(), "a".into());
    store.register_proxy("bg_b".into(), "b".into());

    let snaps = store.snapshot_running();
    let ids: Vec<&str> = snaps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["bg_a", "bg_b", "bg_c"]);
}

#[test]
fn excludes_failed_tasks() {
    let store = make_store();
    store.register_proxy("bg_ok".into(), "ok".into());
    let handle = store.register_proxy("bg_fail".into(), "fail".into());
    handle.complete("err".into(), false);

    let snaps = store.snapshot_running();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].id, "bg_ok");
}

#[test]
fn snapshot_carries_description() {
    let store = make_store();
    store.register_proxy("bg_x".into(), "compiling project".into());

    let snaps = store.snapshot_running();
    assert_eq!(snaps[0].description, "compiling project");
}
