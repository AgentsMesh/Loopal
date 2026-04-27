//! Tick loop persistence tests — durable save on fire/expire, single
//! save per tick, dirty-flag retry on failure.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use loopal_scheduler::{
    CronScheduler, ManualClock, PersistError, PersistedTask, SessionScopedCronStorage,
};

/// Mock store tracking save order + optional first-call failure.
struct TickStore {
    saves: Mutex<Vec<Vec<PersistedTask>>>,
    fail_next: AtomicBool,
    fail_count: AtomicUsize,
}

impl TickStore {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            saves: Mutex::new(Vec::new()),
            fail_next: AtomicBool::new(false),
            fail_count: AtomicUsize::new(0),
        })
    }
    fn arm_failure(&self) {
        self.fail_next.store(true, Ordering::SeqCst);
    }
    async fn save_count(&self) -> usize {
        self.saves.lock().await.len()
    }
    async fn last_save(&self) -> Vec<PersistedTask> {
        self.saves.lock().await.last().cloned().unwrap_or_default()
    }
}

#[async_trait]
impl SessionScopedCronStorage for TickStore {
    async fn load(&self, _session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
        Ok(Vec::new())
    }
    async fn save_all(
        &self,
        _session_id: &str,
        tasks: &[PersistedTask],
    ) -> Result<(), PersistError> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            self.fail_count.fetch_add(1, Ordering::SeqCst);
            return Err(PersistError::Io(std::io::Error::other("armed failure")));
        }
        self.saves.lock().await.push(tasks.to_vec());
        Ok(())
    }
}

async fn build(store: Arc<TickStore>, clock: Arc<ManualClock>) -> Arc<CronScheduler> {
    let store_dyn: Arc<dyn SessionScopedCronStorage> = store;
    let sched = Arc::new(CronScheduler::with_session_storage_and_clock(
        store_dyn, clock,
    ));
    sched.switch_session("tick-test").await.unwrap();
    sched
}

async fn pump_ticks(clock: &ManualClock, to: chrono::DateTime<chrono::Utc>, rounds: usize) {
    clock.set(to);
    for _ in 0..rounds {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn recurring_durable_fire_persists_last_fired() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let store = TickStore::new();
    let sched = build(store.clone(), clock.clone()).await;
    sched.add("* * * * *", "p", true, true).await.expect("add");
    // `add` with durable=true issues one initial save.
    let initial_saves = store.save_count().await;
    assert_eq!(initial_saves, 1);

    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();
    sched.start(tx, cancel.clone());

    pump_ticks(
        &clock,
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 1, 5).unwrap(),
        3,
    )
    .await;
    let _trigger = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("fire")
        .expect("open");

    assert!(store.save_count().await > initial_saves, "fire must save");
    let last = store.last_save().await;
    assert_eq!(last.len(), 1, "one task survives");
    assert!(
        last[0].last_fired_unix_ms.is_some(),
        "last_fired must be persisted"
    );
    cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn oneshot_durable_fire_removes_from_store() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let store = TickStore::new();
    let sched = build(store.clone(), clock.clone()).await;
    sched
        .add("* * * * *", "once", false, true)
        .await
        .expect("add");

    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();
    sched.start(tx, cancel.clone());

    pump_ticks(
        &clock,
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 1, 5).unwrap(),
        3,
    )
    .await;
    let _trigger = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("fire")
        .expect("open");

    assert!(store.last_save().await.is_empty(), "one-shot removed");
    cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn multiple_fires_in_one_tick_save_once() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let store = TickStore::new();
    let sched = build(store.clone(), clock.clone()).await;
    // Three durable tasks all firing at the same minute.
    sched.add("* * * * *", "a", true, true).await.expect("add");
    sched.add("* * * * *", "b", true, true).await.expect("add");
    sched.add("* * * * *", "c", true, true).await.expect("add");
    let baseline = store.save_count().await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();
    sched.start(tx, cancel.clone());

    pump_ticks(
        &clock,
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 1, 5).unwrap(),
        3,
    )
    .await;
    // Drain triggers.
    for _ in 0..3 {
        let _ = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
    }

    // Exactly one additional save for this tick — not three.
    let after = store.save_count().await;
    assert_eq!(after - baseline, 1, "batched fires must save once");
    cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn save_failure_retries_on_next_tick() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let store = TickStore::new();
    let sched = build(store.clone(), clock.clone()).await;
    // First save (from add) will fail.
    store.arm_failure();
    sched.add("* * * * *", "p", true, true).await.expect("add");
    assert_eq!(store.fail_count.load(Ordering::SeqCst), 1);
    // No successful save yet.
    assert_eq!(store.save_count().await, 0);

    let (tx, _rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();
    sched.start(tx, cancel.clone());

    // One tick with no firing task still retries because dirty=true.
    pump_ticks(
        &clock,
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 45).unwrap(),
        3,
    )
    .await;

    assert!(
        store.save_count().await >= 1,
        "dirty flag must trigger retry"
    );
    cancel.cancel();
}
