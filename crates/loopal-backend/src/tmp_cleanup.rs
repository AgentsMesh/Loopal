use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) const LOOPAL_TMP_DIR: &str = "loopal";

// reason: defends against path traversal + injection (NUL / control chars).
// The same guard runs at log file creation (log_writer) so both ends agree.
pub fn is_valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains('/')
        && !id.contains('\\')
        && id != "."
        && id != ".."
        && !id.chars().any(|c| c.is_ascii_control())
}

pub fn session_tmp_root(session_id: &str) -> PathBuf {
    session_tmp_root_in(&loopal_tmp_root(), session_id)
}

pub fn session_tmp_root_in(root: &Path, session_id: &str) -> PathBuf {
    root.join(session_id)
}

pub fn loopal_tmp_root() -> PathBuf {
    std::env::temp_dir().join(LOOPAL_TMP_DIR)
}

/// Remove the entire session tmp dir, except files listed in `exclude`.
///
/// Used by SessionEnd / ClearHistory / PreCompact / on-demand cleanup hooks.
/// `exclude` is a set of paths that must NOT be deleted (typically still-running
/// background tasks' log files). Failure is logged but never raised.
pub async fn cleanup_session_tmp(session_id: &str, exclude: &[PathBuf]) {
    cleanup_session_tmp_in(&loopal_tmp_root(), session_id, exclude).await;
}

// reason: explicit `root` parameter lets tests target an isolated tempdir
// instead of the shared `$TMPDIR/loopal/`, eliminating cross-test TOCTOU
// races on the shared root.
pub async fn cleanup_session_tmp_in(root: &Path, session_id: &str, exclude: &[PathBuf]) {
    if !is_valid_session_id(session_id) {
        tracing::warn!(session_id, "skip cleanup: invalid session id");
        return;
    }
    let session_root = session_tmp_root_in(root, session_id);
    if exclude.is_empty() {
        if let Err(e) = tokio::fs::remove_dir_all(&session_root).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(error = %e, path = %session_root.display(), "cleanup_session_tmp failed");
        }
        return;
    }
    let keep: HashSet<&Path> = exclude.iter().map(|p| p.as_path()).collect();
    selective_cleanup(&session_root, &keep).await;
}

async fn selective_cleanup(root: &Path, keep: &HashSet<&Path>) {
    let bash_dir = root.join("bash");
    if prune_dir_except(&bash_dir, keep).await {
        let _ = tokio::fs::remove_dir(&bash_dir).await;
        let _ = tokio::fs::remove_dir(root).await;
    }
}

// reason: returns true if dir is empty (or vanished) after pruning — caller
// uses this to decide whether to remove the dir, avoiding a second read_dir.
async fn prune_dir_except(dir: &Path, keep: &HashSet<&Path>) -> bool {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
        Err(e) => {
            tracing::warn!(error = %e, path = %dir.display(), "read_dir failed");
            return false;
        }
    };
    let mut remaining = 0usize;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let p = entry.path();
        if keep.contains(p.as_path()) {
            remaining += 1;
            continue;
        }
        if let Err(e) = tokio::fs::remove_file(&p).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(error = %e, path = %p.display(), "remove_file failed");
            remaining += 1;
        }
    }
    remaining == 0
}

pub async fn cleanup_orphans(live_sessions: &HashSet<String>) {
    cleanup_orphans_in(&loopal_tmp_root(), live_sessions).await
}

pub async fn cleanup_orphans_in(root: &Path, live_sessions: &HashSet<String>) {
    let mut entries = match tokio::fs::read_dir(root).await {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            tracing::warn!(error = %e, "cleanup_orphans read_dir failed");
            return;
        }
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let Some(id) = name.to_str() else { continue };
        if !is_valid_session_id(id) || live_sessions.contains(id) {
            continue;
        }
        let p = entry.path();
        if let Err(e) = tokio::fs::remove_dir_all(&p).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(error = %e, path = %p.display(), "orphan cleanup failed");
        }
    }
}
