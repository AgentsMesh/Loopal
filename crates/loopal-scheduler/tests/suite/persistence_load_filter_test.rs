//! `load_persisted` filter tests — expired / missed-one-shot dropped;
//! clean entries rehydrated into memory; survivors are re-saved once
//! so the on-disk set stays consistent.
//!
//! Clamp / lifetime / capacity / clean-load behavior lives in the
//! sibling `persistence_load_clamp_test` module.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use tokio::sync::Mutex;

use loopal_scheduler::{
    Clock, CronScheduler, DurableStore, ManualClock, PersistError, PersistedTask,
};

/// In-memory mock store — captures save calls, returns a preset load.
struct MockStore {
    preset: Mutex<Vec<PersistedTask>>,
    saved: Mutex<Vec<Vec<PersistedTask>>>,
}

impl MockStore {
    fn new(preset: Vec<PersistedTask>) -> Arc<Self> {
        Arc::new(Self {
            preset: Mutex::new(preset),
            saved: Mutex::new(Vec::new()),
        })
    }

    async fn save_count(&self) -> usize {
        self.saved.lock().await.len()
    }
}

#[async_trait]
impl DurableStore for MockStore {
    async fn load(&self) -> Result<Vec<PersistedTask>, PersistError> {
        Ok(self.preset.lock().await.clone())
    }
    async fn save_all(&self, tasks: &[PersistedTask]) -> Result<(), PersistError> {
        self.saved.lock().await.push(tasks.to_vec());
        Ok(())
    }
}

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

fn scheduler_with(store: Arc<MockStore>, clock: Arc<dyn Clock>) -> CronScheduler {
    let store_dyn: Arc<dyn DurableStore> = store;
    CronScheduler::with_store_and_clock(store_dyn, clock)
}

#[tokio::test]
async fn load_persisted_without_store_returns_zero() {
    let sched = CronScheduler::new();
    assert_eq!(sched.load_persisted().await.unwrap(), 0);
}

#[tokio::test]
async fn missed_one_shot_is_dropped() {
    // Created 1h ago; one-shot at +5min would have fired 55 min ago.
    let store = MockStore::new(vec![persisted(
        "miss",
        "5 11 * * *", // 11:05 — before the 12:00 base_time
        false,
        -60 * 60,
    )]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock);
    let count = sched.load_persisted().await.unwrap();
    assert_eq!(count, 0);
    assert_eq!(store.save_count().await, 1);
}

#[tokio::test]
async fn recurring_task_is_rehydrated() {
    let store = MockStore::new(vec![persisted("keep", "*/5 * * * *", true, -60)]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock);
    let count = sched.load_persisted().await.unwrap();
    assert_eq!(count, 1);
    let tasks = sched.list().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "keep");
    assert!(tasks[0].durable);
}

#[tokio::test]
async fn future_one_shot_is_kept() {
    // Created "just now" (-5s) with a cron that fires in the future.
    let store = MockStore::new(vec![persisted("once", "5 13 * * *", false, -5)]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock);
    let count = sched.load_persisted().await.unwrap();
    assert_eq!(count, 1);
    let t = sched.list().await;
    assert_eq!(t[0].id, "once");
    assert!(!t[0].recurring);
}

#[tokio::test]
async fn fired_one_shot_is_dropped() {
    let mut p = persisted("fired", "*/5 * * * *", false, -10);
    p.last_fired_unix_ms = Some(base_time().timestamp_millis() - 30_000);
    let store = MockStore::new(vec![p]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock);
    let count = sched.load_persisted().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn unparsable_cron_is_dropped_with_warning() {
    let store = MockStore::new(vec![persisted("bad", "not a cron", true, -60)]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock);
    let count = sched.load_persisted().await.unwrap();
    assert_eq!(count, 0);
}
