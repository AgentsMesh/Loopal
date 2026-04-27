//! `FileScopedCronStore` — session-scoped file backend implementing
//! [`SessionScopedCronStorage`](crate::persistence_session::SessionScopedCronStorage).
//!
//! Stores each session's cron tasks at `<sessions_root>/<id>/cron.json`.
//! One store instance serves all sessions under the root; the `session_id`
//! is supplied per call rather than baked into construction.
//!
//! Atomic writes and quarantine on corruption use the same helpers as
//! [`crate::persistence_file::FileDurableStore`], so on-disk format and
//! crash semantics are identical between the two implementations. Existing
//! `cron.json` files written by `FileDurableStore` load unchanged here.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::json_file_io::{quarantine_path, read_or_empty, write_atomic};
use crate::persistence::{
    LoadedPayload, PersistError, PersistedTask, classify_payload, encode_payload,
};
use crate::persistence_session::SessionScopedCronStorage;

/// File-backed session-scoped cron storage rooted at `sessions_root`.
///
/// Path layout: `<sessions_root>/<session_id>/cron.json`. The session
/// directory is created on first `save_all` for that session; reads of
/// missing data return an empty list so fresh sessions start cleanly.
pub struct FileScopedCronStore {
    sessions_root: PathBuf,
}

impl FileScopedCronStore {
    /// Create a store rooted at `sessions_root` (typically `~/.loopal/sessions`).
    pub fn new(sessions_root: PathBuf) -> Self {
        Self { sessions_root }
    }

    /// Inspect the configured root (for tests / diagnostics).
    pub fn root(&self) -> &Path {
        &self.sessions_root
    }

    fn path_for(&self, session_id: &str) -> PathBuf {
        self.sessions_root.join(session_id).join("cron.json")
    }
}

#[async_trait]
impl SessionScopedCronStorage for FileScopedCronStore {
    async fn load(&self, session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
        let path = self.path_for(session_id);
        let bytes = read_or_empty(&path).await?;
        match classify_payload(&bytes) {
            LoadedPayload::Empty => Ok(Vec::new()),
            LoadedPayload::Tasks(t) => Ok(t),
            LoadedPayload::Quarantine(reason) => {
                quarantine_path(&path, &reason).await?;
                Ok(Vec::new())
            }
        }
    }

    async fn save_all(
        &self,
        session_id: &str,
        tasks: &[PersistedTask],
    ) -> Result<(), PersistError> {
        let bytes = encode_payload(tasks)?;
        write_atomic(&self.path_for(session_id), &bytes).await
    }
}
