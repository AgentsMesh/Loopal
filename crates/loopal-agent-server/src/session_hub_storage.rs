//! Storage-singleton accessors on [`SessionHub`] — split from
//! [`crate::session_hub`] so that file stays focused on the session
//! registry, while this one handles the file-backed cron / task
//! storage lazy-init + root-mismatch policy.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use loopal_agent::{FileScopedTaskStore, SessionScopedTaskStorage};
use loopal_scheduler::{FileScopedCronStore, SessionScopedCronStorage};

use crate::session_hub::SessionHub;

/// Errors returned by [`SessionHub`] storage accessors.
#[derive(Debug)]
#[non_exhaustive]
pub enum SessionHubError {
    /// First storage init committed a different `sessions_root` than the
    /// caller is now passing. Caller decides whether to abort or recover.
    RootMismatch {
        committed: PathBuf,
        requested: PathBuf,
    },
    /// Initial bind of the task store failed with an underlying I/O
    /// error. `sessions_root` is the directory the caller asked us to
    /// commit so the user can correlate the failure with their config.
    TaskStoreBind {
        sessions_root: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for SessionHubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionHubError::RootMismatch {
                committed,
                requested,
            } => write!(
                f,
                "session hub storage root mismatch: committed {:?} but caller passed {:?}",
                committed, requested
            ),
            SessionHubError::TaskStoreBind {
                sessions_root,
                source,
            } => write!(
                f,
                "task store initial bind failed at {:?}: {}",
                sessions_root, source
            ),
        }
    }
}

impl Error for SessionHubError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SessionHubError::TaskStoreBind { source, .. } => Some(source),
            SessionHubError::RootMismatch { .. } => None,
        }
    }
}

impl SessionHub {
    /// Commit the sessions root used by the file-backed storages, or
    /// verify it matches the previously committed value. Returns
    /// [`SessionHubError::RootMismatch`] on mismatch — caller decides
    /// whether to abort the agent setup or fall back to a different
    /// strategy. We don't panic: a misconfigured caller shouldn't take
    /// the whole server down with it.
    pub(crate) async fn commit_storage_root(
        &self,
        sessions_root: &Path,
    ) -> Result<(), SessionHubError> {
        let mut guard = self.storage_root.lock().await;
        match guard.as_ref() {
            Some(existing) if existing == sessions_root => Ok(()),
            Some(existing) => {
                tracing::error!(
                    committed = %existing.display(),
                    requested = %sessions_root.display(),
                    "session hub storage root mismatch"
                );
                Err(SessionHubError::RootMismatch {
                    committed: existing.clone(),
                    requested: sessions_root.to_path_buf(),
                })
            }
            None => {
                *guard = Some(sessions_root.to_path_buf());
                Ok(())
            }
        }
    }

    /// Get (or lazily create) the file-backed cron storage rooted at
    /// `sessions_root`. The first call decides the root; subsequent
    /// calls **must** pass the same root or [`SessionHubError::RootMismatch`]
    /// is returned.
    pub async fn cron_storage(
        &self,
        sessions_root: &Path,
    ) -> Result<Arc<dyn SessionScopedCronStorage>, SessionHubError> {
        self.commit_storage_root(sessions_root).await?;
        let mut guard = self.cron_storage.lock().await;
        if let Some(existing) = guard.as_ref() {
            return Ok(existing.clone());
        }
        let store: Arc<dyn SessionScopedCronStorage> =
            Arc::new(FileScopedCronStore::new(sessions_root.to_path_buf()));
        *guard = Some(store.clone());
        Ok(store)
    }

    /// Get (or lazily create) the file-backed task storage rooted at
    /// `sessions_root`. See [`Self::cron_storage`] for root-mismatch semantics.
    pub async fn task_storage(
        &self,
        sessions_root: &Path,
    ) -> Result<Arc<dyn SessionScopedTaskStorage>, SessionHubError> {
        self.commit_storage_root(sessions_root).await?;
        let mut guard = self.task_storage.lock().await;
        if let Some(existing) = guard.as_ref() {
            return Ok(existing.clone());
        }
        let store: Arc<dyn SessionScopedTaskStorage> =
            Arc::new(FileScopedTaskStore::new(sessions_root.to_path_buf()));
        *guard = Some(store.clone());
        Ok(store)
    }
}
