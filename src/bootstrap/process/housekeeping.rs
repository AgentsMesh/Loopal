pub(super) async fn startup_housekeeping(cwd: &std::path::Path, skip_session_orphan_cleanup: bool) {
    loopal_config::housekeeping::startup_cleanup();
    if let Some(repo_root) = loopal_git::repo_root(cwd) {
        loopal_git::cleanup_stale_worktrees(&repo_root);
    }
    super::discovery::cleanup_stale();
    if !skip_session_orphan_cleanup {
        cleanup_bash_log_orphans().await;
    }
}

pub(crate) fn abbreviate_home(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rel) = path.strip_prefix(&home)
    {
        return format!("~/{}", rel.display());
    }
    path.display().to_string()
}

// Temp session directories without a persisted session are crash leftovers.
pub(super) async fn cleanup_bash_log_orphans() {
    let live_sessions: std::collections::HashSet<String> = match loopal_storage::SessionStore::new()
    {
        Ok(store) => store
            .list_sessions()
            .map(|sessions| sessions.into_iter().map(|session| session.id).collect())
            .unwrap_or_default(),
        Err(_) => std::collections::HashSet::new(),
    };
    loopal_backend::cleanup_orphans(&live_sessions).await;
}
