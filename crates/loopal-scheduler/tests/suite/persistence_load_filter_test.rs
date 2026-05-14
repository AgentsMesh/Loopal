use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};

use loopal_scheduler::{
    Clock, CronScheduler, ManualClock, PersistedTask, SessionScopedCronStorage,
};

use crate::mock_storage::MockStorage;

fn base_time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 10, 12, 0, 0).unwrap()
}

fn persisted(id: &str, cron: &str, recurring: bool, created_shift: i64) -> PersistedTask {
    let t = base_time() + chrono::Duration::seconds(created_shift);
    PersistedTask {
        id: id.into(),
        cron: cron.into(),
        prompt: "p".into(),
        recurring,
        created_at_unix_ms: t.timestamp_millis(),
        last_fired_unix_ms: None,
    }
}

async fn scheduler_with(store: Arc<MockStorage>, clock: Arc<dyn Clock>) -> CronScheduler {
    let store_dyn: Arc<dyn SessionScopedCronStorage> = store;
    CronScheduler::with_session_storage_and_clock(store_dyn, clock)
}

async fn store_with_preset(preset: Vec<PersistedTask>) -> Arc<MockStorage> {
    let store = MockStorage::new();
    store.seed("test", preset).await;
    store
}

#[tokio::test]
async fn load_persisted_without_store_returns_zero() {
    let sched = CronScheduler::new();
    assert_eq!(sched.switch_session("test").await.unwrap(), 0);
}

#[tokio::test]
async fn missed_one_shot_is_dropped() {
    // Created 1h ago; one-shot at +5min would have fired 55 min ago.
    let store = store_with_preset(vec![persisted(
        "miss",
        "5 11 * * *", // 11:05 — before the 12:00 base_time
        false,
        -60 * 60,
    )])
    .await;
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    sched.wait_idle().await;
    assert_eq!(count, 0);
    assert_eq!(store.save_count().await, 1);
}

#[tokio::test]
async fn recurring_task_is_rehydrated() {
    let store = store_with_preset(vec![persisted("keep", "*/5 * * * *", true, -60)]).await;
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 1);
    let tasks = sched.list().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "keep");
    assert!(tasks[0].durable);
}

#[tokio::test]
async fn future_one_shot_is_kept() {
    // Created "just now" (-5s) with a cron that fires in the future.
    let store = store_with_preset(vec![persisted("once", "5 13 * * *", false, -5)]).await;
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 1);
    let t = sched.list().await;
    assert_eq!(t[0].id, "once");
    assert!(!t[0].recurring);
}

#[tokio::test]
async fn fired_one_shot_is_dropped() {
    let mut p = persisted("fired", "*/5 * * * *", false, -10);
    p.last_fired_unix_ms = Some(base_time().timestamp_millis() - 30_000);
    let store = store_with_preset(vec![p]).await;
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn unparsable_cron_is_dropped_with_warning() {
    let store = store_with_preset(vec![persisted("bad", "not a cron", true, -60)]).await;
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn missing_id_is_dropped() {
    // Forward-compat: a task with a default (empty) id must be filtered
    // rather than silently joining the in-memory set.
    let mut p = persisted("placeholder", "*/5 * * * *", true, -60);
    p.id = String::new();
    let store = store_with_preset(vec![p]).await;
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 0);
}
