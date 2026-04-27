//! Tests for `CronScheduler::switch_session` — atomic active-session swap.
//!
//! Covers four state transitions:
//! - None → Some(A): no flush, loads A
//! - Some(A) → Some(B): A flushed, B loaded
//! - Some(A) → Some(A): no-op (verify by counting save_all calls)
//! - Flush failure: returns Ok, dirty=true, doesn't block load

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::Mutex;

use loopal_scheduler::{
    CronScheduler, ManualClock, PersistError, PersistedTask, SessionScopedCronStorage,
};

/// In-memory session-scoped storage with call recording for assertions.
struct RecordingStorage {
    state: Mutex<std::collections::HashMap<String, Vec<PersistedTask>>>,
    saves: Mutex<Vec<(String, Vec<PersistedTask>)>>,
    fail_save_for: Mutex<Option<String>>,
}

impl RecordingStorage {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(Default::default()),
            saves: Mutex::new(Vec::new()),
            fail_save_for: Mutex::new(None),
        })
    }

    async fn seed(&self, session_id: &str, tasks: Vec<PersistedTask>) {
        self.state.lock().await.insert(session_id.into(), tasks);
    }

    async fn arm_save_failure(&self, session_id: &str) {
        *self.fail_save_for.lock().await = Some(session_id.into());
    }

    async fn save_count(&self) -> usize {
        self.saves.lock().await.len()
    }

    async fn saves_for(&self, session_id: &str) -> usize {
        self.saves
            .lock()
            .await
            .iter()
            .filter(|(s, _)| s == session_id)
            .count()
    }
}

#[async_trait]
impl SessionScopedCronStorage for RecordingStorage {
    async fn load(&self, session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
        Ok(self
            .state
            .lock()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn save_all(
        &self,
        session_id: &str,
        tasks: &[PersistedTask],
    ) -> Result<(), PersistError> {
        let arm = self.fail_save_for.lock().await.clone();
        if let Some(target) = arm
            && target == session_id
        {
            *self.fail_save_for.lock().await = None;
            return Err(PersistError::Io(std::io::Error::other("forced")));
        }
        self.saves
            .lock()
            .await
            .push((session_id.into(), tasks.to_vec()));
        self.state
            .lock()
            .await
            .insert(session_id.into(), tasks.to_vec());
        Ok(())
    }
}

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
    let storage = RecordingStorage::new();
    storage
        .seed("alpha", vec![task("a1", "*/5 * * * *", "hi", now)])
        .await;
    let scheduler = CronScheduler::with_session_storage_and_clock(storage.clone(), clock);
    assert_eq!(storage.save_count().await, 0);
    let n = scheduler.switch_session("alpha").await.expect("switch");
    assert_eq!(n, 1);
    assert_eq!(scheduler.list().await.len(), 1);
    // No save fired during the switch — only a load.
    assert_eq!(storage.save_count().await, 0);
}

#[tokio::test]
async fn switch_between_sessions_flushes_old_and_loads_new() {
    let (clock, now) = frozen_clock();
    let storage = RecordingStorage::new();
    storage
        .seed("alpha", vec![task("a1", "*/5 * * * *", "in alpha", now)])
        .await;
    storage
        .seed("beta", vec![task("b1", "0 9 * * *", "in beta", now)])
        .await;
    let scheduler = CronScheduler::with_session_storage_and_clock(storage.clone(), clock);
    scheduler.switch_session("alpha").await.unwrap();
    assert_eq!(scheduler.list().await[0].id, "a1");
    let baseline = storage.save_count().await;
    scheduler.switch_session("beta").await.unwrap();
    let listed = scheduler.list().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "b1");
    // A flush against alpha must have happened during the swap.
    assert!(storage.saves_for("alpha").await >= 1);
    assert!(storage.save_count().await > baseline);
}

#[tokio::test]
async fn switch_to_same_session_is_noop() {
    let (clock, now) = frozen_clock();
    let storage = RecordingStorage::new();
    storage
        .seed("s", vec![task("s1", "*/5 * * * *", "hi", now)])
        .await;
    let scheduler = CronScheduler::with_session_storage_and_clock(storage.clone(), clock);
    scheduler.switch_session("s").await.unwrap();
    let baseline = storage.save_count().await;
    let n = scheduler.switch_session("s").await.unwrap();
    assert_eq!(n, 0, "no-op must report zero loaded");
    assert_eq!(storage.save_count().await, baseline, "no save on no-op");
}

#[tokio::test]
async fn flush_failure_does_not_block_switch() {
    let (clock, now) = frozen_clock();
    let storage = RecordingStorage::new();
    storage
        .seed("alpha", vec![task("a1", "*/5 * * * *", "hi", now)])
        .await;
    storage
        .seed("beta", vec![task("b1", "0 9 * * *", "hi", now)])
        .await;
    let scheduler = CronScheduler::with_session_storage_and_clock(storage.clone(), clock);
    scheduler.switch_session("alpha").await.unwrap();
    storage.arm_save_failure("alpha").await;
    // Flush against alpha will fail; switch must still load beta.
    let n = scheduler.switch_session("beta").await.unwrap();
    assert_eq!(n, 1);
    assert_eq!(scheduler.list().await[0].id, "b1");
}

#[tokio::test]
async fn unbound_scheduler_switch_is_noop() {
    let (clock, _) = frozen_clock();
    // No storage attached.
    let scheduler = CronScheduler::with_clock(clock);
    let n = scheduler.switch_session("any").await.unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn switch_resets_store_disabled_for_new_session() {
    // After a corrupt-load on session A, store_disabled latches; switching
    // to B must clear it so saves/loads against B work normally.
    let (clock, now) = frozen_clock();
    struct FailingLoadOnce {
        target: Mutex<Option<String>>,
        inner: Arc<RecordingStorage>,
    }
    #[async_trait]
    impl SessionScopedCronStorage for FailingLoadOnce {
        async fn load(&self, session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
            if self.target.lock().await.as_deref() == Some(session_id) {
                *self.target.lock().await = None;
                return Err(PersistError::BadCron("forced".into()));
            }
            self.inner.load(session_id).await
        }
        async fn save_all(
            &self,
            session_id: &str,
            tasks: &[PersistedTask],
        ) -> Result<(), PersistError> {
            self.inner.save_all(session_id, tasks).await
        }
    }
    let inner = RecordingStorage::new();
    inner
        .seed("beta", vec![task("b1", "*/5 * * * *", "hi", now)])
        .await;
    let storage: Arc<FailingLoadOnce> = Arc::new(FailingLoadOnce {
        target: Mutex::new(Some("alpha".into())),
        inner,
    });
    let scheduler = CronScheduler::with_session_storage_and_clock(storage.clone(), clock);
    let _ = scheduler.switch_session("alpha").await;
    let n = scheduler.switch_session("beta").await.expect("beta loads");
    assert_eq!(n, 1);
    let _ = std::any::type_name::<RecordingStorage>();
    let _ = AtomicBool::new(false).load(Ordering::SeqCst);
}
