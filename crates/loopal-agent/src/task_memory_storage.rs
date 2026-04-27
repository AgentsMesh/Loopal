//! In-memory `SessionScopedTaskStorage` impl for sub-agent / ephemeral
//! task lists.
//!
//! Sub-agents (`depth > 0`) and any caller that wants a task store with
//! no on-disk side effects should construct
//! [`TaskStore::with_session_storage`](crate::task_store::TaskStore::with_session_storage)
//! around an [`InMemoryTaskStorage`]. Save/load operate on a per-session
//! `HashMap` cache — switching sessions still works, but state never
//! reaches the filesystem and is dropped when the storage instance does.
//!
//! Mirrors [`loopal_scheduler::CronScheduler::new`]'s in-memory cron
//! semantics so cron + task share a single "ephemeral" policy at the
//! sub-agent boundary.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::task_session_storage::{SessionScopedTaskStorage, TaskLoad};
use crate::types::Task;

/// Type alias for the per-session cache: tasks + next id.
type SessionCache = HashMap<String, (Vec<Task>, u64)>;

/// In-memory task storage. Each session id maps to its own `(tasks, next_id)`
/// snapshot so `switch_session` round-trips work as expected.
///
/// Uses `std::sync::Mutex` because the critical section is a single
/// `HashMap::get` / `HashMap::insert` — no `await`, no I/O. A poisoned
/// lock (caller panicked mid-mutation) is recovered via `into_inner` so
/// the storage stays usable for the rest of the test/session rather
/// than propagating the panic up the async stack.
#[derive(Default)]
pub struct InMemoryTaskStorage {
    sessions: Mutex<SessionCache>,
}

impl InMemoryTaskStorage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lock the inner map, recovering the inner state if the previous
    /// holder panicked (poisoned lock). Storage semantics are
    /// "best-effort cache" — a panic during a previous `save_all` does
    /// not invalidate the rest of the in-memory state, so we keep going
    /// rather than re-panic.
    fn lock_recover(&self) -> std::sync::MutexGuard<'_, SessionCache> {
        match self.sessions.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[async_trait]
impl SessionScopedTaskStorage for InMemoryTaskStorage {
    async fn load(&self, session_id: &str) -> std::io::Result<TaskLoad> {
        Ok(self
            .lock_recover()
            .get(session_id)
            .cloned()
            .unwrap_or_else(|| (Vec::new(), 1)))
    }

    async fn save_all(&self, session_id: &str, tasks: &[Task]) -> std::io::Result<()> {
        let next_id = tasks
            .iter()
            .filter_map(|t| t.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        self.lock_recover()
            .insert(session_id.into(), (tasks.to_vec(), next_id));
        Ok(())
    }
}
