//! Tests for `CronScheduler` durable add / remove persistence hooks.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;

use loopal_scheduler::{CronScheduler, DurableStore, ManualClock, PersistError, PersistedTask};

struct CountingStore {
    saved: Mutex<Vec<Vec<PersistedTask>>>,
    fail_next: AtomicBool,
}

impl CountingStore {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            saved: Mutex::new(Vec::new()),
            fail_next: AtomicBool::new(false),
        })
    }
    fn arm_failure(&self) {
        self.fail_next.store(true, Ordering::SeqCst);
    }
    async fn save_count(&self) -> usize {
        self.saved.lock().await.len()
    }
    async fn last_ids(&self) -> Vec<String> {
        self.saved
            .lock()
            .await
            .last()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|t| t.id)
            .collect()
    }
}

#[async_trait]
impl DurableStore for CountingStore {
    async fn load(&self) -> Result<Vec<PersistedTask>, PersistError> {
        Ok(Vec::new())
    }
    async fn save_all(&self, tasks: &[PersistedTask]) -> Result<(), PersistError> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(PersistError::Io(std::io::Error::other("armed failure")));
        }
        self.saved.lock().await.push(tasks.to_vec());
        Ok(())
    }
}

fn build_scheduler(store: Arc<CountingStore>) -> CronScheduler {
    let store_dyn: Arc<dyn DurableStore> = store;
    CronScheduler::with_store_and_clock(store_dyn, Arc::new(ManualClock::new(Utc::now())))
}

#[tokio::test]
async fn add_non_durable_does_not_persist() {
    let store = CountingStore::new();
    let sched = build_scheduler(store.clone());
    sched
        .add("*/5 * * * *", "p", true, false)
        .await
        .expect("add");
    assert_eq!(store.save_count().await, 0);
}

#[tokio::test]
async fn add_durable_triggers_one_save() {
    let store = CountingStore::new();
    let sched = build_scheduler(store.clone());
    let id = sched
        .add("*/5 * * * *", "p", true, true)
        .await
        .expect("add");
    assert_eq!(store.save_count().await, 1);
    assert_eq!(store.last_ids().await, vec![id]);
}

#[tokio::test]
async fn remove_durable_persists_new_set() {
    let store = CountingStore::new();
    let sched = build_scheduler(store.clone());
    let id = sched
        .add("*/5 * * * *", "p", true, true)
        .await
        .expect("add");
    assert!(sched.remove(&id).await);
    // One save on add, one on remove.
    assert_eq!(store.save_count().await, 2);
    assert!(store.last_ids().await.is_empty());
}

#[tokio::test]
async fn remove_non_durable_does_not_persist() {
    let store = CountingStore::new();
    let sched = build_scheduler(store.clone());
    let id = sched
        .add("*/5 * * * *", "p", true, false)
        .await
        .expect("add");
    assert!(sched.remove(&id).await);
    assert_eq!(store.save_count().await, 0);
}

#[tokio::test]
async fn snapshot_includes_only_durable_tasks() {
    let store = CountingStore::new();
    let sched = build_scheduler(store.clone());
    let _a = sched
        .add("*/5 * * * *", "non", true, false)
        .await
        .expect("add");
    let b = sched
        .add("*/7 * * * *", "dur", true, true)
        .await
        .expect("add");
    // The durable save must only carry `b`.
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
    let store = CountingStore::new();
    let sched = build_scheduler(store.clone());
    store.arm_failure();
    let durable_id = sched
        .add("*/5 * * * *", "persist", true, true)
        .await
        .expect("add durable");
    // Armed failure was consumed; no successful save yet.
    assert_eq!(store.save_count().await, 0);

    // A non-durable add should still trigger a retry because dirty=true.
    let _transient = sched
        .add("*/7 * * * *", "transient", true, false)
        .await
        .expect("add transient");
    assert_eq!(store.save_count().await, 1);
    // The persisted set still contains only the durable task.
    assert_eq!(store.last_ids().await, vec![durable_id]);
}
