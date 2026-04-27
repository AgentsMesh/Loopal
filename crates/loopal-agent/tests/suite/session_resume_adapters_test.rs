//! Tests for `CronResumeAdapter` / `TaskResumeAdapter` — verify the
//! `SessionResumeHook` notification fans out to the underlying
//! `switch_session` calls.

use std::sync::Arc;

use async_trait::async_trait;
use loopal_agent::types::{Task, TaskStatus};
use loopal_agent::{
    CronResumeAdapter, FileScopedTaskStore, SessionScopedTaskStorage, TaskResumeAdapter, TaskStore,
};
use loopal_runtime::SessionResumeHook;
use loopal_scheduler::{CronScheduler, PersistError, PersistedTask, SessionScopedCronStorage};
use tempfile::tempdir;
use tokio::sync::Mutex;

/// In-memory cron storage that records loads to verify the adapter
/// triggered an actual `switch_session` (which in turn calls `load`).
struct CronProbe {
    loads: Mutex<Vec<String>>,
}

impl CronProbe {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            loads: Mutex::new(Vec::new()),
        })
    }
    async fn load_count_for(&self, sid: &str) -> usize {
        self.loads
            .lock()
            .await
            .iter()
            .filter(|s| s.as_str() == sid)
            .count()
    }
}

#[async_trait]
impl SessionScopedCronStorage for CronProbe {
    async fn load(&self, session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
        self.loads.lock().await.push(session_id.into());
        Ok(Vec::new())
    }
    async fn save_all(&self, _: &str, _: &[PersistedTask]) -> Result<(), PersistError> {
        Ok(())
    }
}

#[tokio::test]
async fn cron_adapter_forwards_to_switch_session() {
    let probe = CronProbe::new();
    let scheduler = Arc::new(CronScheduler::with_session_storage(probe.clone()));
    let adapter = CronResumeAdapter::new(scheduler.clone());
    adapter.on_session_changed("alpha").await.unwrap();
    assert_eq!(probe.load_count_for("alpha").await, 1);
    adapter.on_session_changed("beta").await.unwrap();
    assert_eq!(probe.load_count_for("beta").await, 1);
}

#[tokio::test]
async fn task_adapter_forwards_to_switch_session() {
    let dir = tempdir().unwrap();
    let storage: Arc<dyn SessionScopedTaskStorage> =
        Arc::new(FileScopedTaskStore::new(dir.path().to_path_buf()));
    // Seed beta on disk so we can detect the switch.
    storage
        .save_all(
            "beta",
            &[Task {
                id: "1".into(),
                subject: "in-beta".into(),
                description: String::new(),
                active_form: None,
                status: TaskStatus::Pending,
                owner: None,
                blocked_by: Vec::new(),
                blocks: Vec::new(),
                metadata: serde_json::Value::Object(Default::default()),
                created_at: "2026-04-26T00:00:00Z".into(),
            }],
        )
        .await
        .unwrap();
    let task_store = Arc::new(TaskStore::with_session_storage(storage));
    let adapter = TaskResumeAdapter::new(task_store.clone());
    assert!(task_store.list().await.is_empty());
    adapter.on_session_changed("beta").await.unwrap();
    let listed = task_store.list().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].subject, "in-beta");
}

#[tokio::test]
async fn cron_adapter_unbound_scheduler_is_noop() {
    // No storage attached → switch_session returns 0, adapter doesn't panic.
    let scheduler = Arc::new(CronScheduler::new());
    let adapter = CronResumeAdapter::new(scheduler);
    adapter.on_session_changed("any").await.unwrap();
}
