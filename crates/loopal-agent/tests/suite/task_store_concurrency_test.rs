//! Concurrency tests for `TaskStore` — verifies that disk persistence
//! does NOT hold the in-memory lock, so reads/writes proceed even while
//! a slow `save_all` is in flight.
//!
//! This guards against regression of the original lock-within-await
//! pattern (write lock held across `storage.save_all().await`) which
//! blocked every other CRUD on the store until disk I/O finished.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use loopal_agent::types::Task;
use loopal_agent::{SessionScopedTaskStorage, TaskStore};
use tokio::sync::Notify;

/// Storage that blocks the first `save_all` call on a `Notify` so the
/// test can interleave a `list()` against the slow save and assert the
/// list returns immediately.
struct BlockingSaveStorage {
    gate: Arc<Notify>,
    state: AtomicU8,
}

impl BlockingSaveStorage {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            gate: Arc::new(Notify::new()),
            state: AtomicU8::new(0),
        })
    }
    fn release(&self) {
        self.gate.notify_one();
    }
    fn state(&self) -> u8 {
        self.state.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl SessionScopedTaskStorage for BlockingSaveStorage {
    async fn load(&self, _: &str) -> std::io::Result<(Vec<Task>, u64)> {
        Ok((Vec::new(), 1))
    }
    async fn save_all(&self, _: &str, _: &[Task]) -> std::io::Result<()> {
        self.state.store(1, Ordering::SeqCst);
        self.gate.notified().await;
        self.state.store(2, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_does_not_block_on_in_flight_save() {
    let storage = BlockingSaveStorage::new();
    let store = Arc::new(TaskStore::with_session_storage(storage.clone()));
    store.switch_session("s1").await.unwrap();

    // Kick off create — its persist will block at `save_all`.
    let store_for_create = store.clone();
    let create_handle = tokio::spawn(async move {
        store_for_create.create("blocking", "").await;
    });

    // Wait until save is observably in-flight (state 1).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while storage.state() != 1 {
        if tokio::time::Instant::now() >= deadline {
            panic!("save did not start within timeout");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    // While save is blocked: list() must return promptly. If the write
    // lock was still held, this would hang until `release()`.
    let listed = tokio::time::timeout(Duration::from_millis(500), store.list())
        .await
        .expect("list() must return while save is in flight (lock not held across await)");
    assert_eq!(listed.len(), 1, "the just-created task should be visible");

    // Release the save and let create finish.
    storage.release();
    create_handle.await.unwrap();
    assert_eq!(storage.state(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_creates_serialize_persist_without_lost_updates() {
    // Plain in-memory storage — no blocking. We only assert that
    // concurrent creates don't lose updates and the final list reflects
    // every committed task.
    let storage: Arc<dyn SessionScopedTaskStorage> =
        Arc::new(loopal_agent::InMemoryTaskStorage::new());
    let store = Arc::new(TaskStore::with_session_storage(storage));
    store.switch_session("s").await.unwrap();

    // Spawn 16 concurrent creates.
    let mut handles = Vec::new();
    for i in 0..16 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            s.create(&format!("t{i}"), "").await;
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let listed = store.list().await;
    assert_eq!(listed.len(), 16, "every concurrent create must persist");
}
