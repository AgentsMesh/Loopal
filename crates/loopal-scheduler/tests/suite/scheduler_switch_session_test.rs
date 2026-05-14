use std::sync::Arc;

use chrono::{DateTime, Utc};

use loopal_scheduler::{CronScheduler, ManualClock, PersistedTask, SessionScopedCronStorage};

use crate::mock_storage::MockStorage;

fn frozen_clock() -> (Arc<ManualClock>, DateTime<Utc>) {
    let now = Utc::now();
    (Arc::new(ManualClock::new(now)), now)
}

fn task(id: &str, cron: &str, prompt: &str, created: DateTime<Utc>) -> PersistedTask {
    PersistedTask {
        id: id.into(),
        cron: cron.into(),
        prompt: prompt.into(),
        recurring: true,
        created_at_unix_ms: created.timestamp_millis(),
        last_fired_unix_ms: Some(created.timestamp_millis()),
    }
}

#[tokio::test]
async fn unbound_to_session_loads_without_flushing() {
    let (clock, now) = frozen_clock();
    let storage = MockStorage::new();
    storage
        .seed("alpha", vec![task("a1", "*/5 * * * *", "hi", now)])
        .await;
    let store_dyn: Arc<dyn SessionScopedCronStorage> = storage.clone();
    let scheduler = CronScheduler::with_session_storage_and_clock(store_dyn, clock);
    assert_eq!(storage.save_count().await, 0);
    let n = scheduler.switch_session("alpha").await.expect("switch");
    assert_eq!(n, 1);
    assert_eq!(scheduler.list().await.len(), 1);
    assert_eq!(storage.save_count().await, 0);
}

#[tokio::test]
async fn switch_between_sessions_flushes_old_and_loads_new() {
    let (clock, now) = frozen_clock();
    let storage = MockStorage::new();
    storage
        .seed("alpha", vec![task("a1", "*/5 * * * *", "in alpha", now)])
        .await;
    storage
        .seed("beta", vec![task("b1", "0 9 * * *", "in beta", now)])
        .await;
    let store_dyn: Arc<dyn SessionScopedCronStorage> = storage.clone();
    let scheduler = CronScheduler::with_session_storage_and_clock(store_dyn, clock);
    scheduler.switch_session("alpha").await.unwrap();
    scheduler.wait_idle().await;
    assert_eq!(scheduler.list().await[0].id, "a1");
    let baseline = storage.save_count().await;
    scheduler.switch_session("beta").await.unwrap();
    scheduler.wait_idle().await;
    let listed = scheduler.list().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "b1");
    assert!(storage.saves_for("alpha").await >= 1);
    assert!(storage.save_count().await > baseline);
}

#[tokio::test]
async fn switch_to_same_session_is_noop() {
    let (clock, now) = frozen_clock();
    let storage = MockStorage::new();
    storage
        .seed("s", vec![task("s1", "*/5 * * * *", "hi", now)])
        .await;
    let store_dyn: Arc<dyn SessionScopedCronStorage> = storage.clone();
    let scheduler = CronScheduler::with_session_storage_and_clock(store_dyn, clock);
    scheduler.switch_session("s").await.unwrap();
    let baseline = storage.save_count().await;
    let n = scheduler.switch_session("s").await.unwrap();
    assert_eq!(n, 0, "no-op must report zero loaded");
    assert_eq!(storage.save_count().await, baseline, "no save on no-op");
}

#[tokio::test]
async fn flush_failure_does_not_block_switch() {
    let (clock, now) = frozen_clock();
    let storage = MockStorage::new();
    storage
        .seed("alpha", vec![task("a1", "*/5 * * * *", "hi", now)])
        .await;
    storage
        .seed("beta", vec![task("b1", "0 9 * * *", "hi", now)])
        .await;
    let store_dyn: Arc<dyn SessionScopedCronStorage> = storage.clone();
    let scheduler = CronScheduler::with_session_storage_and_clock(store_dyn, clock);
    scheduler.switch_session("alpha").await.unwrap();
    storage.arm_save_failure_for("alpha").await;
    let n = scheduler.switch_session("beta").await.unwrap();
    assert_eq!(n, 1);
    assert_eq!(scheduler.list().await[0].id, "b1");
}

#[tokio::test]
async fn unbound_scheduler_switch_is_noop() {
    let (clock, _) = frozen_clock();
    let scheduler = CronScheduler::with_clock(clock);
    let n = scheduler.switch_session("any").await.unwrap();
    assert_eq!(n, 0);
}
