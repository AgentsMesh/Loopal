use std::time::Duration;

use crate::test_support::{make_store, spawn_completed_task, spawn_raw};

#[tokio::test]
async fn subscribe_then_register_sends_notification() {
    let store = make_store();
    let mut rx = store.subscribe_spawns();
    let pid = spawn_raw(&store, "echo one").await;
    let notif = rx.recv().await.expect("notification should arrive");
    assert_eq!(notif.task_id, pid);
    assert!(notif.description.contains("echo one"));
}

#[tokio::test]
async fn insert_without_subscriber_does_not_panic() {
    let store = make_store();
    let _ = spawn_completed_task(&store, "").await;
}

#[tokio::test]
async fn multiple_subscribers_receive_independently() {
    let store = make_store();
    let mut rx1 = store.subscribe_spawns();
    let mut rx2 = store.subscribe_spawns();
    let pid = spawn_raw(&store, "echo a").await;
    let n1 = rx1.recv().await.unwrap();
    let n2 = rx2.recv().await.unwrap();
    assert_eq!(n1.task_id, pid);
    assert_eq!(n2.task_id, pid);
}

#[tokio::test]
async fn ordered_delivery_for_sequential_registrations() {
    let store = make_store();
    let mut rx = store.subscribe_spawns();
    let pa = spawn_raw(&store, "echo a").await;
    let pb = spawn_raw(&store, "echo b").await;
    let pc = spawn_raw(&store, "echo c").await;
    let mut ids = Vec::new();
    for _ in 0..3 {
        ids.push(rx.recv().await.unwrap().task_id);
    }
    assert_eq!(ids, vec![pa, pb, pc]);
}

#[tokio::test]
async fn dropping_receiver_does_not_block_inserts() {
    let store = make_store();
    let rx = store.subscribe_spawns();
    drop(rx);
    let pid = spawn_raw(&store, "echo done").await;
    let present = store.read_task(&pid, |t| t.description().to_string());
    assert!(present.is_some());

    use loopal_tool_background::ops::bg_output;
    let _ = bg_output(&store, &pid, true, Duration::from_secs(2)).await;
}
