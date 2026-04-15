/// Tests for SpawnNotification — subscribe_spawns() and insert() notification.
use std::sync::Arc;

use loopal_tool_background::BackgroundTaskStore;

fn make_store() -> Arc<BackgroundTaskStore> {
    BackgroundTaskStore::new()
}

#[test]
fn subscribe_then_insert_sends_notification() {
    let store = make_store();
    let mut rx = store.subscribe_spawns();
    store.register_proxy("bg_1".into(), "task one".into());
    let notif = rx.try_recv().expect("should receive notification");
    assert_eq!(notif.task_id, "bg_1");
    assert_eq!(notif.description, "task one");
}

#[test]
fn insert_without_subscriber_does_not_panic() {
    let store = make_store();
    store.register_proxy("bg_1".into(), "no subscriber".into());
}

#[test]
fn multiple_inserts_send_ordered_notifications() {
    let store = make_store();
    let mut rx = store.subscribe_spawns();
    store.register_proxy("bg_a".into(), "a".into());
    store.register_proxy("bg_b".into(), "b".into());
    store.register_proxy("bg_c".into(), "c".into());
    let ids: Vec<String> = (0..3).map(|_| rx.try_recv().unwrap().task_id).collect();
    assert_eq!(ids, vec!["bg_a", "bg_b", "bg_c"]);
}

#[test]
fn notification_carries_live_arc_handles() {
    let store = make_store();
    let mut rx = store.subscribe_spawns();
    let proxy = store.register_proxy("bg_1".into(), "task".into());
    let notif = rx.try_recv().unwrap();

    assert!(notif.output.lock().unwrap().is_empty());
    proxy.complete("final output".into(), true);
    let output = notif.output.lock().unwrap().clone();
    assert_eq!(output, "final output");
}

#[test]
fn subscriber_dropped_insert_still_works() {
    let store = make_store();
    let rx = store.subscribe_spawns();
    drop(rx);
    store.register_proxy("bg_1".into(), "after drop".into());
    assert!(store.with_task("bg_1", |_| true).unwrap());
}

#[test]
fn status_watch_in_notification_receives_updates() {
    let store = make_store();
    let mut rx = store.subscribe_spawns();
    let proxy = store.register_proxy("bg_1".into(), "test".into());
    let notif = rx.try_recv().unwrap();

    use loopal_tool_background::TaskStatus;
    assert_eq!(*notif.status_watch.borrow(), TaskStatus::Running);
    proxy.complete("done".into(), false);
    let mut watch = notif.status_watch;
    assert_eq!(*watch.borrow_and_update(), TaskStatus::Failed);
}
