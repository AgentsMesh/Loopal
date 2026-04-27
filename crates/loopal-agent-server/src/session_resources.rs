//! Per-session resource construction for the agent setup pipeline.
//!
//! Builds three things that all key off the same session id:
//! - the [`TaskStore`]
//! - the [`CronScheduler`]
//! - the [`SessionResumeHook`] adapters that fan out a `ResumeSession`
//!   control command to whichever of the above two need to follow the
//!   agent across a session swap
//!
//! ## Sub-agent ephemeral policy
//!
//! Root agents (`depth = 0`) get **file-backed** storage: cron + task
//! state survives across process restarts and follows the user across
//! `/resume`. Sub-agents (`depth > 0`) get **in-memory-only** storage
//! for both cron and task: their `session_id` is a transient uuid that
//! nobody resumes, so persisting would only litter `~/.loopal/sessions/`
//! with orphan files. The `cron_bridge` / `task_bridge` only observe
//! the root agent's stores anyway.
//!
//! Resume hooks are registered for both stores at every depth — they are
//! cheap no-ops when the underlying state is in-memory, and uniformly
//! letting a sub-agent re-bind to a new session id keeps the trait
//! contract simple ("resume always touches everything").
//!
//! Extracted from `agent_setup` so the builder there stays readable —
//! these four facts (sessions_root, task store, scheduler, hooks) form
//! one cohesive unit and change together when adding a new session-
//! scoped resource.

use std::path::PathBuf;
use std::sync::Arc;

use loopal_agent::{
    CronResumeAdapter, InMemoryTaskStorage, SessionScopedTaskStorage, TaskResumeAdapter, TaskStore,
};
use loopal_runtime::SessionResumeHook;
use loopal_scheduler::CronScheduler;

/// Bundle returned by [`build_session_scoped_resources`].
pub(crate) struct SessionScopedResources {
    pub task_store: Arc<TaskStore>,
    pub scheduler: Arc<CronScheduler>,
    pub resume_hooks: Vec<Arc<dyn SessionResumeHook>>,
}

/// Build the task store, scheduler, and resume hooks for the agent.
///
/// `sessions_root` is the directory under which each `<session_id>/`
/// subdirectory lives (file-backed only at depth 0). The task store is
/// bound immediately to `session_id`; the scheduler is **not** — its
/// bind is async (`switch_session(...).await`) and runs in the caller
/// right after this returns.
///
/// File-backed storages are taken from the [`SessionHub`] singletons so
/// every root agent in this process shares a single
/// `FileScopedCronStore` / `FileScopedTaskStore` instance. Sub-agents
/// (`depth > 0`) get their own ephemeral [`InMemoryTaskStorage`] +
/// in-memory [`CronScheduler`] — they don't need to coordinate with
/// other agents and persisting their session would litter disk.
pub(crate) async fn build_session_scoped_resources(
    hub: &crate::session_hub::SessionHub,
    sessions_root: PathBuf,
    session_id: &str,
    depth: u32,
) -> Result<SessionScopedResources, crate::session_hub_storage::SessionHubError> {
    let is_root = depth == 0;

    let task_storage: Arc<dyn SessionScopedTaskStorage> = if is_root {
        hub.task_storage(&sessions_root).await?
    } else {
        Arc::new(InMemoryTaskStorage::new())
    };
    let task_store = Arc::new(TaskStore::with_session_storage(task_storage));
    // Initial bind has no previous session to flush, so the only error
    // path is the storage backend itself failing to load — typically an
    // `io::Error` from the file scope or a panic-recovered `Default`
    // from the in-memory scope. We surface it as `TaskStoreBind` rather
    // than panic so the agent setup pipeline can report it cleanly.
    task_store
        .switch_session(session_id)
        .await
        .map_err(
            |source| crate::session_hub_storage::SessionHubError::TaskStoreBind {
                sessions_root: sessions_root.clone(),
                source,
            },
        )?;

    let scheduler = if is_root {
        let cron_storage = hub.cron_storage(&sessions_root).await?;
        Arc::new(CronScheduler::with_session_storage(cron_storage))
    } else {
        Arc::new(CronScheduler::new())
    };

    let resume_hooks: Vec<Arc<dyn SessionResumeHook>> = vec![
        Arc::new(CronResumeAdapter::new(scheduler.clone())),
        Arc::new(TaskResumeAdapter::new(task_store.clone())),
    ];

    Ok(SessionScopedResources {
        task_store,
        scheduler,
        resume_hooks,
    })
}

/// Resolve the sessions root directory.
///
/// Tests pass `session_dir_override` to redirect into a tempdir; in
/// production we use `loopal_config::sessions_dir()` (typically
/// `~/.loopal/sessions/`), with a `temp_dir()/loopal/sessions` fallback
/// for sandboxed environments where home directory resolution fails.
///
/// `pub` (rather than `pub(crate)`) so the testing module can re-export
/// for cross-crate test coverage.
pub fn resolve_sessions_root(session_dir_override: Option<&std::path::Path>) -> PathBuf {
    session_dir_override
        .map(|p| p.to_path_buf())
        .or_else(|| loopal_config::sessions_dir().ok())
        .unwrap_or_else(|| std::env::temp_dir().join("loopal/sessions"))
}
