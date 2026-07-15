mod error;
mod files;
mod git_command;
mod git_diff;
mod git_mutations;
mod git_ops;
mod git_parse;
pub mod git_types;
mod git_validate;
mod notification;
mod path_guard;
mod search;
mod sessions;
pub mod types;
mod watch;
mod working_directory;
mod working_directory_cleanup;
mod working_directory_prepare_cleanup;
#[cfg(test)]
mod working_directory_prepare_safety_tests;
#[cfg(test)]
mod working_directory_tests;

use std::path::Path;
use std::sync::Arc;

use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_storage::SessionStore;
use tokio::sync::{Mutex as AsyncMutex, broadcast};

pub use error::WorkspaceError;
pub use notification::ServiceNotification;
pub use path_guard::RootGuard;
pub use sessions::DesktopSession;
pub use working_directory::{
    PreparedWorkingDirectory, WorkingDirectoryInfo, inspect_working_directory,
    prepare_worktree_directory,
};
pub use working_directory_cleanup::{CleanedWorkingDirectory, cleanup_prepared_worktree};

pub const LOCAL_WORKSPACE_ID: &str = "local-workspace";

pub struct WorkspaceService {
    pub(crate) workspace_id: String,
    pub(crate) guard: RootGuard,
    pub(crate) backend: Arc<LocalBackend>,
    pub(crate) write_lock: AsyncMutex<()>,
    pub(crate) events: broadcast::Sender<ServiceNotification>,
    pub(crate) session_store: Option<Arc<SessionStore>>,
    _watcher: Option<watch::WatcherHandle>,
}

impl WorkspaceService {
    pub fn new(root: impl AsRef<Path>) -> Result<Arc<Self>, WorkspaceError> {
        Self::build(root, SessionStore::new().ok().map(Arc::new))
    }

    pub fn with_session_store(
        root: impl AsRef<Path>,
        session_store: SessionStore,
    ) -> Result<Arc<Self>, WorkspaceError> {
        Self::build(root, Some(Arc::new(session_store)))
    }

    fn build(
        root: impl AsRef<Path>,
        session_store: Option<Arc<SessionStore>>,
    ) -> Result<Arc<Self>, WorkspaceError> {
        let guard = RootGuard::new(root)?;
        let (events, _) = broadcast::channel(512);
        let watcher = match watch::start(guard.clone(), LOCAL_WORKSPACE_ID.into(), events.clone()) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                tracing::warn!(%error, "workspace file watching unavailable");
                None
            }
        };
        Ok(Arc::new(Self {
            workspace_id: LOCAL_WORKSPACE_ID.into(),
            backend: LocalBackend::new(
                guard.root().to_path_buf(),
                None,
                ResourceLimits::default(),
                "desktop-workspace",
            ),
            guard,
            write_lock: AsyncMutex::new(()),
            events,
            session_store,
            _watcher: watcher,
        }))
    }

    pub fn root(&self) -> &Path {
        self.guard.root()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServiceNotification> {
        self.events.subscribe()
    }

    pub fn require_workspace(&self, workspace_id: &str) -> Result<(), WorkspaceError> {
        if workspace_id == self.workspace_id {
            Ok(())
        } else {
            Err(WorkspaceError::new(
                "workspace_not_found",
                format!("unknown workspace: {workspace_id}"),
            ))
        }
    }
}
