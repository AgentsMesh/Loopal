//! Session-scoped storage trait for the agent task list.
//!
//! Mirrors [`SessionScopedCronStorage`](loopal_scheduler::SessionScopedCronStorage)
//! including the `Result`-typed load — both surface I/O and serde errors
//! to the caller instead of silently falling back to empty state.

use async_trait::async_trait;

use crate::types::Task;

/// Output of a successful `load` — the persisted task list and the
/// lowest unused integer ID (max numeric task ID + 1, or 1 if empty).
pub type TaskLoad = (Vec<Task>, u64);

#[async_trait]
pub trait SessionScopedTaskStorage: Send + Sync {
    /// Missing storage (file/dir absent) is `Ok((vec![], 1))`, not an
    /// error. Errors signal genuine I/O or parse failures the caller
    /// should surface.
    async fn load(&self, session_id: &str) -> std::io::Result<TaskLoad>;

    /// Replace the stored set for `session_id` with `tasks`.
    async fn save_all(&self, session_id: &str, tasks: &[Task]) -> std::io::Result<()>;
}
