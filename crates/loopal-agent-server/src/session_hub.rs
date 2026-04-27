//! Shared session registry for multi-client access.
//!
//! `SharedSession` + `ClientHandle` live in [`crate::shared_session`].
//! Storage-singleton accessors (`cron_storage` / `task_storage` /
//! `commit_storage_root`) and the [`SessionHubError`](crate::session_hub_storage::SessionHubError)
//! type live in [`crate::session_hub_storage`]. This file owns only
//! the registry struct + session lifecycle methods.

#![allow(dead_code)] // agent/join + agent/list methods used when wired into dispatch_loop

use std::sync::Arc;

use loopal_agent::SessionScopedTaskStorage;
use loopal_scheduler::SessionScopedCronStorage;
use tokio::sync::Mutex;

pub use crate::shared_session::{ClientHandle, SharedSession};

/// Server-wide session registry.
///
/// ## Lock ordering
///
/// `SessionHub` owns six independent `tokio::sync::Mutex` fields. The
/// only path that ever holds two of them simultaneously is the storage
/// accessors (in [`crate::session_hub_storage`]), which observe a
/// strict order:
///
/// 1. `storage_root` (acquired in `commit_storage_root`)
/// 2. `cron_storage` **xor** `task_storage` (never both at once)
///
/// The other three (`sessions`, `test_provider`, `session_dir_override`)
/// are independent — no method takes more than one of them at a time.
///
/// **Future maintainers**: if you add a method that needs to hold two
/// of these locks together, document the order here and follow the
/// existing convention. Reversing `cron_storage`/`task_storage` and
/// `storage_root` would risk deadlock against concurrent callers
/// already holding the other.
#[derive(Default)]
pub struct SessionHub {
    pub(crate) sessions: Mutex<Vec<Arc<SharedSession>>>,
    /// Test-only: injected mock provider for session creation.
    pub(crate) test_provider: Mutex<Option<Arc<dyn loopal_provider_api::Provider>>>,
    /// Override base directory for session/message storage (test sandboxes).
    pub(crate) session_dir_override: Mutex<Option<std::path::PathBuf>>,
    /// Lazy-initialized file-backed cron storage shared by every root
    /// agent in this server. Eliminates the "one `FileScopedCronStore`
    /// per `build_session_scoped_resources` call" duplication that
    /// existed in earlier revisions and matches the architectural
    /// invariant "Layer 1 storage is a process-wide singleton".
    pub(crate) cron_storage: Mutex<Option<Arc<dyn SessionScopedCronStorage>>>,
    /// Counterpart for the agent task list.
    pub(crate) task_storage: Mutex<Option<Arc<dyn SessionScopedTaskStorage>>>,
    /// Sessions root committed at first storage init. Subsequent calls
    /// to `cron_storage` / `task_storage` return [`SessionHubError::RootMismatch`]
    /// (in [`crate::session_hub_storage`]) if a caller passes a different
    /// root — silent root mismatch would let two agent setups share
    /// storage they think is independent.
    pub(crate) storage_root: Mutex<Option<std::path::PathBuf>>,
}

impl SessionHub {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(Vec::new()),
            test_provider: Mutex::new(None),
            session_dir_override: Mutex::new(None),
            cron_storage: Mutex::new(None),
            task_storage: Mutex::new(None),
            storage_root: Mutex::new(None),
        }
    }

    /// Set a mock provider for testing (consumed on next session creation).
    pub async fn set_test_provider(&self, provider: Arc<dyn loopal_provider_api::Provider>) {
        *self.test_provider.lock().await = Some(provider);
    }

    /// Get the test provider (if set). Cloned — available for multiple sessions.
    pub async fn get_test_provider(&self) -> Option<Arc<dyn loopal_provider_api::Provider>> {
        self.test_provider.lock().await.clone()
    }

    /// Override session storage directory (for sandbox/test environments).
    pub async fn set_session_dir_override(&self, dir: std::path::PathBuf) {
        *self.session_dir_override.lock().await = Some(dir);
    }

    /// Get the session directory override, if set.
    pub async fn session_dir_override(&self) -> Option<std::path::PathBuf> {
        self.session_dir_override.lock().await.clone()
    }

    /// Register a new session.
    pub async fn register_session(&self, session: Arc<SharedSession>) {
        self.sessions.lock().await.push(session);
    }

    /// Find a session by ID.
    pub async fn find_session(&self, id: &str) -> Option<Arc<SharedSession>> {
        self.sessions
            .lock()
            .await
            .iter()
            .find(|s| s.session_id == id)
            .cloned()
    }

    /// List all active session IDs.
    pub async fn list_session_ids(&self) -> Vec<String> {
        self.sessions
            .lock()
            .await
            .iter()
            .map(|s| s.session_id.clone())
            .collect()
    }

    /// Remove a session when the agent loop completes.
    pub async fn remove_session(&self, id: &str) {
        self.sessions.lock().await.retain(|s| s.session_id != id);
    }
}
