//! Tests for `TaskStore::switch_session` — atomic active-session swap.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_agent::types::{Task, TaskStatus};
use loopal_agent::{SessionScopedTaskStorage, TaskStore};

/// In-memory recording storage for assertions.
struct RecordingStorage {
    state: Mutex<std::collections::HashMap<String, (Vec<Task>, u64)>>,
    saves: Mutex<Vec<(String, Vec<Task>)>>,
}

impl RecordingStorage {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(Default::default()),
            saves: Mutex::new(Vec::new()),
        })
    }
    fn seed(&self, session_id: &str, tasks: Vec<Task>) {
        let next_id = tasks
            .iter()
            .filter_map(|t| t.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        self.state
            .lock()
            .unwrap()
            .insert(session_id.into(), (tasks, next_id));
    }
    fn saves_for(&self, session_id: &str) -> usize {
        self.saves
            .lock()
            .unwrap()
            .iter()
            .filter(|(s, _)| s == session_id)
            .count()
    }
    fn save_count(&self) -> usize {
        self.saves.lock().unwrap().len()
    }
}

#[async_trait]
impl SessionScopedTaskStorage for RecordingStorage {
    async fn load(&self, session_id: &str) -> std::io::Result<(Vec<Task>, u64)> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .get(session_id)
            .cloned()
            .unwrap_or_else(|| (Vec::new(), 1)))
    }
    async fn save_all(&self, session_id: &str, tasks: &[Task]) -> std::io::Result<()> {
        self.saves
            .lock()
            .unwrap()
            .push((session_id.into(), tasks.to_vec()));
        let next_id = tasks
            .iter()
            .filter_map(|t| t.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        self.state
            .lock()
            .unwrap()
            .insert(session_id.into(), (tasks.to_vec(), next_id));
        Ok(())
    }
}

fn task(id: &str, subject: &str) -> Task {
    Task {
        id: id.into(),
        subject: subject.into(),
        description: String::new(),
        active_form: None,
        status: TaskStatus::Pending,
        owner: None,
        blocked_by: Vec::new(),
        blocks: Vec::new(),
        metadata: serde_json::Value::Object(Default::default()),
        created_at: "2026-04-26T00:00:00Z".into(),
    }
}

#[tokio::test]
async fn unbound_to_session_loads_without_flushing() {
    let storage = RecordingStorage::new();
    storage.seed("alpha", vec![task("1", "in alpha")]);
    let store = TaskStore::with_session_storage(storage.clone());
    assert_eq!(storage.save_count(), 0);
    store.switch_session("alpha").await.unwrap();
    let listed = store.list().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].subject, "in alpha");
    assert_eq!(storage.save_count(), 0, "no flush on initial bind");
}

#[tokio::test]
async fn switch_between_sessions_flushes_old_loads_new() {
    let storage = RecordingStorage::new();
    storage.seed("alpha", vec![task("1", "alpha-task")]);
    storage.seed("beta", vec![task("1", "beta-task")]);
    let store = TaskStore::with_session_storage(storage.clone());
    store.switch_session("alpha").await.unwrap();
    // Mutate alpha so flush has a different snapshot.
    store.create("new in alpha", "").await;
    let baseline = storage.save_count();
    store.switch_session("beta").await.unwrap();
    let listed = store.list().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].subject, "beta-task");
    assert!(storage.saves_for("alpha") >= 1, "must flush alpha");
    assert!(storage.save_count() > baseline);
}

#[tokio::test]
async fn switch_to_same_session_is_noop() {
    let storage = RecordingStorage::new();
    storage.seed("s", vec![task("1", "x")]);
    let store = TaskStore::with_session_storage(storage.clone());
    store.switch_session("s").await.unwrap();
    let baseline = storage.save_count();
    store.switch_session("s").await.unwrap();
    assert_eq!(storage.save_count(), baseline, "no-op must not save");
}

#[tokio::test]
async fn unbound_create_drops_on_first_switch() {
    let storage = RecordingStorage::new();
    let store = TaskStore::with_session_storage(storage.clone());
    // Create before switch — goes to memory, doesn't persist.
    store.create("ephemeral", "").await;
    assert_eq!(storage.save_count(), 0);
    storage.seed("real", vec![task("1", "persisted")]);
    store.switch_session("real").await.unwrap();
    let listed = store.list().await;
    // After load, only the persisted task remains.
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].subject, "persisted");
}

#[tokio::test]
async fn switch_notifies_subscribers() {
    let storage = RecordingStorage::new();
    storage.seed("alpha", vec![task("1", "x")]);
    let store = TaskStore::with_session_storage(storage.clone());
    let mut rx = store.subscribe();
    store.switch_session("alpha").await.unwrap();
    // Notification must be available after switch.
    let _: () = rx.try_recv().expect("must receive change notification");
}

/// Storage that fails the next `save_all` once, then succeeds. Verifies
/// `TaskStore::switch_session`'s "half-success" contract: flush error
/// surfaces but the new session is still loaded and observers are notified.
struct FailingFlushStorage {
    inner: Arc<RecordingStorage>,
    fail_next: AtomicBool,
}

impl FailingFlushStorage {
    fn new(inner: Arc<RecordingStorage>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            fail_next: AtomicBool::new(false),
        })
    }
    fn arm(&self) {
        self.fail_next.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl SessionScopedTaskStorage for FailingFlushStorage {
    async fn load(&self, session_id: &str) -> std::io::Result<(Vec<Task>, u64)> {
        self.inner.load(session_id).await
    }
    async fn save_all(&self, session_id: &str, tasks: &[Task]) -> std::io::Result<()> {
        // Stream G: record every attempt (incl. failures) so the recording
        // mock's `saves` log preserves a complete audit trail.
        let _ = self.inner.save_all(session_id, tasks).await;
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(std::io::Error::other("armed flush failure"));
        }
        Ok(())
    }
}

#[tokio::test]
async fn flush_failure_surfaces_err_but_loads_new_session_and_notifies() {
    let inner = RecordingStorage::new();
    inner.seed("alpha", vec![task("1", "alpha")]);
    inner.seed("beta", vec![task("1", "beta")]);
    let storage = FailingFlushStorage::new(inner.clone());
    let store = TaskStore::with_session_storage(storage.clone());
    store.switch_session("alpha").await.unwrap();
    // Mutate alpha so the swap has something to flush.
    store.create("transient", "").await;
    let mut rx = store.subscribe();
    storage.arm();
    let result = store.switch_session("beta").await;
    assert!(result.is_err(), "armed flush failure must surface");
    // Despite the error, the new session loaded.
    let listed = store.list().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].subject, "beta");
    // Observers must still be notified after a half-success swap so the
    // task bridge re-snapshots even when flush fails.
    let _: () = rx
        .try_recv()
        .expect("notify_change must fire even when flush failed");
}

#[tokio::test]
async fn subsequent_operations_work_after_half_success_swap() {
    let inner = RecordingStorage::new();
    inner.seed("alpha", vec![task("1", "alpha")]);
    let storage = FailingFlushStorage::new(inner.clone());
    let store = TaskStore::with_session_storage(storage.clone());
    store.switch_session("alpha").await.unwrap();
    storage.arm();
    let _ = store.switch_session("beta").await; // half-success
    // Subsequent create / update on the new session must still work and
    // persist (storage is no longer armed to fail).
    let t = store.create("post-swap", "").await;
    let listed = store.list().await;
    assert!(listed.iter().any(|x| x.id == t.id));
}
