//! End-to-end durability: first scheduler adds durable jobs, writes
//! them to a real `FileDurableStore`; a second scheduler pointing at
//! the same file calls `load_persisted` and rehydrates them.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use loopal_scheduler::{CronScheduler, DurableStore, FileDurableStore, ManualClock};
use tempfile::tempdir;

#[tokio::test]
async fn durable_tasks_survive_scheduler_restart() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cron.json");

    // --- Writer scheduler ----------------------------------------
    let store_a: Arc<dyn DurableStore> = Arc::new(FileDurableStore::new(path.clone()));
    let clock_a = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 0).unwrap(),
    ));
    let writer = CronScheduler::with_store_and_clock(store_a, clock_a);
    let durable_id = writer
        .add("*/5 * * * *", "stay", true, true)
        .await
        .expect("durable add");
    let _transient = writer
        .add("*/7 * * * *", "vanish", true, false)
        .await
        .expect("transient add");

    // Drop the writer — simulates a restart.
    drop(writer);

    // --- Reader scheduler ----------------------------------------
    let store_b: Arc<dyn DurableStore> = Arc::new(FileDurableStore::new(path.clone()));
    let clock_b = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 2, 0).unwrap(),
    ));
    let reader = CronScheduler::with_store_and_clock(store_b, clock_b);
    let count = reader.load_persisted().await.expect("load");

    assert_eq!(count, 1, "only the durable task rehydrates");
    let tasks = reader.list().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, durable_id);
    assert_eq!(tasks[0].prompt, "stay");
    assert!(tasks[0].durable);
}

#[tokio::test]
async fn load_persisted_cleans_up_missed_one_shot() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cron.json");
    let store_a: Arc<dyn DurableStore> = Arc::new(FileDurableStore::new(path.clone()));
    // Created at 10:00; one-shot at 10:05.
    let clock_a = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 0).unwrap(),
    ));
    let sched = CronScheduler::with_store_and_clock(store_a, clock_a);
    sched
        .add("5 10 * * *", "once", false, true)
        .await
        .expect("add");
    drop(sched);

    // Reload at 11:00 — one-shot time is in the past → dropped.
    let store_b: Arc<dyn DurableStore> = Arc::new(FileDurableStore::new(path.clone()));
    let clock_b = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 4, 10, 11, 0, 0).unwrap(),
    ));
    let reader = CronScheduler::with_store_and_clock(store_b, clock_b);
    let count = reader.load_persisted().await.expect("load");
    assert_eq!(count, 0);
    assert!(reader.list().await.is_empty());

    // File should be cleaned up after reload.
    let raw = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(
        raw.contains(r#""tasks": []"#),
        "cleanup save must rewrite empty set, got: {raw}"
    );
}
