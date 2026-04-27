//! R2: tests for `store_disabled` — when `load_persisted` fails, the
//! scheduler must refuse to persist afterwards so later writes don't
//! clobber an unrecognized on-disk file.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;

use loopal_scheduler::{
    CronScheduler, ManualClock, PersistError, PersistedTask, SessionScopedCronStorage,
};

/// A store whose `load` always fails — simulates quarantine-failed
/// or unreadable on-disk state.
struct FailingLoadStore {
    saves: Mutex<usize>,
}

impl FailingLoadStore {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            saves: Mutex::new(0),
        })
    }
    async fn save_count(&self) -> usize {
        *self.saves.lock().await
    }
}

#[async_trait]
impl SessionScopedCronStorage for FailingLoadStore {
    async fn load(&self, _session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
        Err(PersistError::Io(std::io::Error::other("unreadable")))
    }
    async fn save_all(
        &self,
        _session_id: &str,
        _tasks: &[PersistedTask],
    ) -> Result<(), PersistError> {
        *self.saves.lock().await += 1;
        Ok(())
    }
}

#[tokio::test]
async fn load_failure_latches_store_disabled_and_skips_subsequent_saves() {
    // If the storage `load` fails (e.g. quarantine could not move a
    // corrupt file aside), the scheduler must **refuse** to persist
    // afterwards, otherwise a later `add` atomically overwrites the
    // user's unrecognized on-disk state with an empty snapshot.
    let store = FailingLoadStore::new();
    let store_dyn: Arc<dyn SessionScopedCronStorage> = store.clone();
    let sched = CronScheduler::with_session_storage_and_clock(
        store_dyn,
        Arc::new(ManualClock::new(Utc::now())),
    );
    let err = sched.switch_session("test").await.unwrap_err();
    assert!(
        matches!(err, PersistError::Io(_)),
        "expected Io error, got {err:?}"
    );

    // Further durable adds must succeed in memory but MUST NOT reach
    // the store — otherwise the corrupt file gets clobbered.
    let _id = sched
        .add("*/5 * * * *", "after-disable", true, true)
        .await
        .expect("in-memory add should succeed");
    assert_eq!(sched.list().await.len(), 1);
    assert_eq!(
        store.save_count().await,
        0,
        "save_all MUST NOT be called once the store is disabled"
    );
}

#[tokio::test]
async fn remove_after_load_failure_also_skips_store() {
    // Same guarantee on the `remove` path — once the store is
    // disabled, neither add nor remove should clobber on-disk data.
    let store = FailingLoadStore::new();
    let store_dyn: Arc<dyn SessionScopedCronStorage> = store.clone();
    let sched = CronScheduler::with_session_storage_and_clock(
        store_dyn,
        Arc::new(ManualClock::new(Utc::now())),
    );
    let _ = sched.switch_session("test").await;

    let id = sched
        .add("*/5 * * * *", "x", true, true)
        .await
        .expect("add");
    assert!(sched.remove(&id).await);
    assert_eq!(
        store.save_count().await,
        0,
        "remove MUST NOT trigger save once the store is disabled"
    );
}
