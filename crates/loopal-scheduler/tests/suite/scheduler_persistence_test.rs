use std::sync::Arc;

use chrono::Utc;

use loopal_scheduler::{CronScheduler, ManualClock, SessionScopedCronStorage};

use crate::mock_storage::MockStorage;

async fn build_scheduler(store: Arc<MockStorage>) -> CronScheduler {
    let store_dyn: Arc<dyn SessionScopedCronStorage> = store;
    let sched = CronScheduler::with_session_storage_and_clock(
        store_dyn,
        Arc::new(ManualClock::new(Utc::now())),
    );
    sched.switch_session("durable-test").await.unwrap();
    sched
}

#[tokio::test]
async fn add_non_durable_does_not_persist() {
    let store = MockStorage::new();
    let sched = build_scheduler(store.clone()).await;
    sched
        .add("*/5 * * * *", "p", true, false)
        .await
        .expect("add");
    assert_eq!(store.save_count().await, 0);
}

#[tokio::test]
async fn add_durable_triggers_one_save() {
    let store = MockStorage::new();
    let sched = build_scheduler(store.clone()).await;
    let id = sched
        .add("*/5 * * * *", "p", true, true)
        .await
        .expect("add");
    sched.wait_idle().await;
    assert_eq!(store.save_count().await, 1);
    assert_eq!(store.last_ids().await, vec![id]);
}

#[tokio::test]
async fn remove_durable_persists_new_set() {
    let store = MockStorage::new();
    let sched = build_scheduler(store.clone()).await;
    let id = sched
        .add("*/5 * * * *", "p", true, true)
        .await
        .expect("add");
    assert!(sched.remove(&id).await);
    sched.wait_idle().await;
    // One save on add, one on remove.
    assert_eq!(store.save_count().await, 2);
    assert!(store.last_ids().await.is_empty());
}

#[tokio::test]
async fn remove_non_durable_does_not_persist() {
    let store = MockStorage::new();
    let sched = build_scheduler(store.clone()).await;
    let id = sched
        .add("*/5 * * * *", "p", true, false)
        .await
        .expect("add");
    assert!(sched.remove(&id).await);
    sched.wait_idle().await;
    assert_eq!(store.save_count().await, 0);
}

#[tokio::test]
async fn snapshot_includes_only_durable_tasks() {
    let store = MockStorage::new();
    let sched = build_scheduler(store.clone()).await;
    let _a = sched
        .add("*/5 * * * *", "non", true, false)
        .await
        .expect("add");
    let b = sched
        .add("*/7 * * * *", "dur", true, true)
        .await
        .expect("add");
    sched.wait_idle().await;
    let ids = store.last_ids().await;
    assert_eq!(ids, vec![b]);
}

#[tokio::test]
async fn list_exposes_durable_flag() {
    let sched = CronScheduler::new();
    let id_a = sched.add("*/5 * * * *", "non", true, false).await.unwrap();
    let id_b = sched.add("*/7 * * * *", "dur", true, true).await.unwrap();
    let tasks = sched.list().await;
    let a = tasks.iter().find(|t| t.id == id_a).unwrap();
    let b = tasks.iter().find(|t| t.id == id_b).unwrap();
    assert!(!a.durable);
    assert!(b.durable);
}

#[tokio::test]
async fn scheduler_without_store_ignores_durable_flag() {
    // No crash when durable=true on an in-memory-only scheduler; the
    // task still lives in memory.
    let sched = CronScheduler::new();
    let id = sched
        .add("*/5 * * * *", "x", true, true)
        .await
        .expect("add");
    assert_eq!(sched.list().await.len(), 1);
    assert_eq!(sched.list().await[0].id, id);
}

#[tokio::test]
async fn subsequent_add_retries_after_save_failure() {
    // First durable save fails → dirty flag latches. A later
    // non-durable add must still retry the save so memory and disk
    // don't diverge indefinitely.
    let store = MockStorage::new();
    let sched = build_scheduler(store.clone()).await;
    store.arm_save_failure();
    let durable_id = sched
        .add("*/5 * * * *", "persist", true, true)
        .await
        .expect("add durable");
    sched.wait_idle().await;
    assert_eq!(store.save_count().await, 0);
    assert_eq!(store.fail_save_attempts(), 1);

    let _transient = sched
        .add("*/7 * * * *", "transient", true, false)
        .await
        .expect("add transient");
    sched.wait_idle().await;
    assert_eq!(store.save_count().await, 1);
    assert_eq!(store.last_ids().await, vec![durable_id]);
}
