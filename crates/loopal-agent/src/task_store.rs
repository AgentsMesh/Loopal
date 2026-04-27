//! File-backed task store with in-memory cache and session-scoped storage.
//!
//! Tasks are persisted via [`SessionScopedTaskStorage`] keyed by the
//! currently active `session_id`.
//!
//! ## Concurrency
//!
//! Two locks, separated by responsibility:
//!
//! - `inner: tokio::sync::RwLock<TaskStoreInner>` — guards the in-memory
//!   task list. Held **only across pure memory CRUD**; never across an
//!   `await` to disk.
//! - `persist_mutex: tokio::sync::Mutex<()>` — serializes `save_all`
//!   calls so concurrent writers don't trample each other's file output
//!   (`std::fs::write` is not multi-process atomic). The persist path
//!   takes a fresh `inner.read()` snapshot under the persist lock so the
//!   on-disk state always reflects the latest committed memory.
//!
//! Construct via [`TaskStore::with_session_storage`] (or the convenience
//! [`TaskStore::with_sessions_root`]). The store starts unbound; call
//! [`TaskStore::switch_session`] to load a specific session's tasks.
//!
//! `TaskPatch` (the partial-update payload for `update`) lives in
//! [`crate::task_patch`].

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock, broadcast};

use crate::task_file_storage::FileScopedTaskStore;
use crate::task_patch::TaskPatch;
use crate::task_session_storage::SessionScopedTaskStorage;
use crate::types::{Task, TaskStatus};

/// Capacity for the change-notification broadcast channel.
///
/// Each subscriber gets an independent receiver; `send()` drops the
/// oldest queued value if every receiver is more than `BROADCAST_CAPACITY`
/// items behind. The notification is just `()` (a "something changed"
/// pulse) so even a `Lagged` consumer's correct response is to
/// re-snapshot — losing intermediate pulses is harmless.
pub const BROADCAST_CAPACITY: usize = 16;

/// File-backed task store with in-memory cache.
pub struct TaskStore {
    storage: Arc<dyn SessionScopedTaskStorage>,
    pub(crate) inner: RwLock<TaskStoreInner>,
    /// Serializes `save_all` calls. Held across the disk I/O `await` so
    /// two concurrent writers don't interleave file writes; releasing
    /// `inner` before acquiring this means readers/writers of the
    /// in-memory state are never blocked on disk I/O.
    persist_mutex: Mutex<()>,
    change_tx: broadcast::Sender<()>,
}

pub(crate) struct TaskStoreInner {
    pub(crate) tasks: Vec<Task>,
    pub(crate) next_id: u64,
    pub(crate) active_session_id: Option<String>,
}

impl TaskStore {
    /// Create a session-scoped task store backed by `storage`. The store
    /// starts unbound — call [`switch_session`](Self::switch_session) to
    /// load a specific session's tasks. Mutations before that point go
    /// to memory only and are dropped on the next switch.
    pub fn with_session_storage(storage: Arc<dyn SessionScopedTaskStorage>) -> Self {
        let (change_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            storage,
            inner: RwLock::new(TaskStoreInner {
                tasks: Vec::new(),
                next_id: 1,
                active_session_id: None,
            }),
            persist_mutex: Mutex::new(()),
            change_tx,
        }
    }

    /// Convenience constructor: build a session-scoped store wired to a
    /// [`FileScopedTaskStore`] rooted at `sessions_root`.
    pub fn with_sessions_root(sessions_root: PathBuf) -> Self {
        let storage: Arc<dyn SessionScopedTaskStorage> =
            Arc::new(FileScopedTaskStore::new(sessions_root));
        Self::with_session_storage(storage)
    }

    /// Create a new task. Returns the created task.
    pub async fn create(&self, subject: &str, description: &str) -> Task {
        let task = {
            let mut inner = self.inner.write().await;
            let id = inner.next_id.to_string();
            inner.next_id += 1;
            let task = Task {
                id,
                subject: subject.to_string(),
                description: description.to_string(),
                active_form: None,
                status: TaskStatus::Pending,
                owner: None,
                blocked_by: Vec::new(),
                blocks: Vec::new(),
                metadata: serde_json::Value::Object(Default::default()),
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            inner.tasks.push(task.clone());
            task
        };
        self.persist_active().await;
        self.notify_change();
        task
    }

    /// Get a task by ID.
    pub async fn get(&self, id: &str) -> Option<Task> {
        let inner = self.inner.read().await;
        inner.tasks.iter().find(|t| t.id == id).cloned()
    }

    /// List all non-deleted tasks.
    pub async fn list(&self) -> Vec<Task> {
        let inner = self.inner.read().await;
        inner
            .tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Deleted)
            .cloned()
            .collect()
    }

    /// Update a task. Returns the updated task or `None` if not found.
    pub async fn update(&self, id: &str, patch: TaskPatch) -> Option<Task> {
        let updated = {
            let mut inner = self.inner.write().await;
            let task = inner.tasks.iter_mut().find(|t| t.id == id)?;
            patch.apply(task);
            task.clone()
        };
        self.persist_active().await;
        self.notify_change();
        Some(updated)
    }

    /// Subscribe to change notifications. Each call returns an
    /// **independent** receiver — multiple subscribers (`task_bridge`,
    /// metrics observers, …) can coexist and each receives every
    /// `notify_change()` pulse independently.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.change_tx.subscribe()
    }

    pub(crate) fn notify_change(&self) {
        // `send()` errors only when there are no receivers — that's
        // expected during the windowed period between store construction
        // and any subscriber connecting. Drop the error.
        let _ = self.change_tx.send(());
    }

    /// Persist the active session's task list under `persist_mutex`, so
    /// concurrent persists serialize against each other and the on-disk
    /// state always reflects the latest committed in-memory snapshot.
    /// Held across the disk `await`; never held while taking `inner`'s
    /// write lock.
    pub(crate) async fn persist_active(&self) {
        let _persist_guard = self.persist_mutex.lock().await;
        let (session_id, tasks) = {
            let inner = self.inner.read().await;
            let Some(sid) = inner.active_session_id.clone() else {
                return;
            };
            (sid, inner.tasks.clone())
        };
        if let Err(e) = self.storage.save_all(&session_id, &tasks).await {
            tracing::error!(error = %e, "task store save failed");
        }
    }

    pub(crate) fn storage(&self) -> &Arc<dyn SessionScopedTaskStorage> {
        &self.storage
    }

    pub(crate) fn persist_mutex(&self) -> &Mutex<()> {
        &self.persist_mutex
    }
}
