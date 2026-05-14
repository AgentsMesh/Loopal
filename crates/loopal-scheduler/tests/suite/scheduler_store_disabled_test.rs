use std::sync::Arc;

use chrono::Utc;

use loopal_scheduler::{CronScheduler, ManualClock, PersistError, SessionScopedCronStorage};

use crate::mock_storage::MockStorage;

#[tokio::test]
async fn load_failure_latches_store_disabled_and_skips_subsequent_saves() {
    // If the storage `load` fails (e.g. quarantine could not move a
    // corrupt file aside), the scheduler must refuse to persist
    // afterwards, otherwise a later `add` atomically overwrites the
    // user's unrecognized on-disk state with an empty snapshot.
    let store = MockStorage::new();
    store.arm_load_failure_always();
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
    let store = MockStorage::new();
    store.arm_load_failure_always();
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
