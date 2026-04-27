//! Tests for R1 (clamp), R5 (durable lifetime), capacity & clean-load
//! rewrite behavior in `load_persisted`.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use tokio::sync::Mutex;

use loopal_scheduler::{
    Clock, CronScheduler, ManualClock, PersistError, PersistedTask, SessionScopedCronStorage,
};

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
    async fn last_saved(&self) -> Vec<PersistedTask> {
        self.saved.lock().await.last().cloned().unwrap_or_default()
    }
}

#[async_trait]
impl SessionScopedCronStorage for MockStore {
    async fn load(&self, _session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
        Ok(self.preset.lock().await.clone())
    }
    async fn save_all(
        &self,
        _session_id: &str,
        tasks: &[PersistedTask],
    ) -> Result<(), PersistError> {
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

async fn scheduler_with(store: Arc<MockStore>, clock: Arc<dyn Clock>) -> CronScheduler {
    let store_dyn: Arc<dyn SessionScopedCronStorage> = store;
    CronScheduler::with_session_storage_and_clock(store_dyn, clock)
}

#[tokio::test]
async fn recurring_last_fired_is_clamped_to_now_on_load() {
    let mut p = persisted("recur", "*/5 * * * *", true, -10 * 60);
    p.last_fired_unix_ms = Some((base_time() - chrono::Duration::minutes(5)).timestamp_millis());
    let store = MockStore::new(vec![p]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 1);
    let tasks = sched.list().await;
    let next = tasks[0].next_fire.expect("has next fire");
    assert!(
        next > base_time(),
        "next_fire must be in the future, got {next} vs base {}",
        base_time()
    );
    assert!(store.save_count().await >= 1);
}

#[tokio::test]
async fn recurring_none_last_fired_with_old_created_at_is_clamped() {
    // R1 regression: `last_fired = None` + old `created_at` previously
    // slipped past the clamp.
    let store = MockStore::new(vec![persisted("recur", "*/5 * * * *", true, -10 * 60)]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    sched.switch_session("test").await.unwrap();
    let tasks = sched.list().await;
    let next = tasks[0].next_fire.expect("has next fire");
    assert!(
        next > base_time(),
        "next_fire must be in the future after clamp; got {next}"
    );
    assert!(store.save_count().await >= 1, "clamp must trigger rewrite");
}

#[tokio::test]
async fn durable_recurring_bypasses_three_day_lifetime() {
    // R5: durable tasks are exempt from the 3-day lifetime cap.
    let store = MockStore::new(vec![persisted(
        "old_but_durable",
        "*/5 * * * *",
        true,
        -5 * 24 * 60 * 60,
    )]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 1, "durable old task must survive lifetime cap");
    assert_eq!(sched.list().await[0].id, "old_but_durable");
}

#[tokio::test]
async fn durable_task_ignores_lifetime_cap() {
    // R5 alternate phrasing — ensures the load path mirrors the
    // underlying `ScheduledTask::is_expired` exemption.
    let store = MockStore::new(vec![persisted(
        "old1",
        "*/5 * * * *",
        true,
        -4 * 24 * 60 * 60,
    )]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(sched.list().await[0].id, "old1");
}

#[tokio::test]
async fn load_exceeding_max_tasks_is_truncated() {
    let mut payload = Vec::with_capacity(60);
    for i in 0..60 {
        payload.push(persisted(
            &format!("id{i:02}"),
            &format!("{} * * * *", i % 60),
            true,
            -60,
        ));
    }
    let store = MockStore::new(payload);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 50);
    assert_eq!(store.save_count().await, 1);
    assert_eq!(store.last_saved().await.len(), 50);
}

#[tokio::test]
async fn clean_load_does_not_rewrite_file() {
    // Crons are chosen so `next_after(created_at)` lies strictly in
    // the future relative to `base_time`, avoiding the R1 clamp that
    // would otherwise dirty a "clean" load.
    let store = MockStore::new(vec![
        persisted("a", "0 13 * * *", true, -60),
        persisted("b", "0 14 * * *", true, -60),
    ]);
    let clock = Arc::new(ManualClock::new(base_time()));
    let sched = scheduler_with(store.clone(), clock).await;
    let count = sched.switch_session("test").await.unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        store.save_count().await,
        0,
        "untouched set must not trigger a cleanup save"
    );
}
