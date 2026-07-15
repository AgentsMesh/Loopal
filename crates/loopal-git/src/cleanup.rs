use std::path::Path;
use std::process::Command;

use crate::worktree::{parse_worktree_list, remove_worktree};

/// Remove stale worktrees from `.loopal/worktrees/` (best-effort).
///
/// Uses `git worktree list --porcelain` to identify directories not tracked
/// by git as active worktrees. Called at startup to prevent orphan accumulation.
///
/// Note: This function is best-effort and will not propagate errors. If the
/// process was killed by SIGKILL or panicked, stale worktrees are cleaned up
/// on the next startup via this function.
pub fn cleanup_stale_worktrees(repo_root: &Path) {
    // Prune git's internal worktree bookkeeping first (removes dangling entries).
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(repo_root)
        .output();

    let wt_base = repo_root.join(".loopal").join("worktrees");
    let Ok(entries) = std::fs::read_dir(&wt_base) else {
        return;
    };

    let (active_paths, _) = parse_worktree_list(repo_root);

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Canonicalize for reliable comparison (handles macOS /tmp → /private/tmp).
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !active_paths.contains(&canonical) {
            // Unknown directories and unmerged branches are retained: startup
            // cleanup has no user authorization for destructive removal.
            let _ = remove_worktree(repo_root, name, false);
        }
    }
}
