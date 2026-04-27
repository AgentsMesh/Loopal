//! Session-scoped storage trait for cron tasks.
//!
//! Unlike [`DurableStore`](crate::persistence::DurableStore), which binds
//! a store instance to a fixed path at construction, this trait treats
//! `session_id` as a parameter — one storage instance reads and writes
//! many sessions' data. Aligns cron persistence with the lookup pattern
//! already used by `SessionStore` / `MessageStore` in `loopal-storage`.
//!
//! Concrete file-backed implementation lives in
//! [`crate::persistence_file_scoped::FileScopedCronStore`].

use async_trait::async_trait;

use crate::persistence::{PersistError, PersistedTask};

/// Abstract session-scoped storage for durable cron tasks.
///
/// Implementations must be safe to share via `Arc` and callable
/// concurrently for distinct `session_id`s. `save_all` fully replaces
/// the stored set for the given session.
#[async_trait]
pub trait SessionScopedCronStorage: Send + Sync {
    /// Load all persisted tasks for `session_id`. Missing data
    /// (file or directory absent) returns an empty vector — first use
    /// of a session is not an error.
    async fn load(&self, session_id: &str) -> Result<Vec<PersistedTask>, PersistError>;

    /// Replace the stored set for `session_id` with `tasks`. Must be
    /// atomic: a reader either sees the previous contents or the new
    /// contents, never partial.
    async fn save_all(&self, session_id: &str, tasks: &[PersistedTask])
    -> Result<(), PersistError>;
}
