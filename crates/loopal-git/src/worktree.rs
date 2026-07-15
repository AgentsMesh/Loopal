use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::GitError;
use crate::gitignore::ensure_worktree_exclude;

pub use crate::worktree_removal::{cleanup_if_clean, remove_worktree};

/// Information about a created worktree.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: String,
    pub name: String,
}

/// Create a new git worktree under `.loopal/worktrees/<name>/`.
///
/// The worktree is created on a new branch `loopal-wt-<name>` based on HEAD.
/// The `.loopal/worktrees/` directory is created if it does not exist.
pub fn create_worktree(repo_root: &Path, name: &str) -> Result<WorktreeInfo, GitError> {
    create_worktree_from(repo_root, name, "HEAD")
}

pub fn create_worktree_at(
    repo_root: &Path,
    name: &str,
    start_point: &str,
) -> Result<WorktreeInfo, GitError> {
    let valid_oid = matches!(start_point.len(), 40 | 64)
        && start_point.bytes().all(|value| value.is_ascii_hexdigit());
    if !valid_oid {
        return Err(GitError::CommandFailed(
            "worktree start point must be a full Git object ID".into(),
        ));
    }
    create_worktree_from(repo_root, name, start_point)
}

fn create_worktree_from(
    repo_root: &Path,
    name: &str,
    start_point: &str,
) -> Result<WorktreeInfo, GitError> {
    validate_name(name)?;

    let wt_dir = repo_root.join(".loopal").join("worktrees").join(name);
    if wt_dir.exists() {
        return Err(GitError::WorktreeExists(name.to_string()));
    }

    let branch = format!("loopal-wt-{name}");
    if local_branch_exists(repo_root, &branch)? {
        return Err(GitError::WorktreeExists(name.to_string()));
    }

    ensure_worktree_exclude(repo_root)?;
    if let Some(parent) = wt_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch])
        .arg(&wt_dir)
        .arg(start_point)
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        if local_branch_exists(repo_root, &branch).unwrap_or(false) {
            return Err(GitError::WorktreeExists(name.to_string()));
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GitError::CommandFailed(stderr));
    }

    Ok(WorktreeInfo {
        path: wt_dir,
        branch,
        name: name.to_string(),
    })
}

fn local_branch_exists(repo_root: &Path, branch: &str) -> Result<bool, GitError> {
    let reference = format!("refs/heads/{branch}");
    let output = Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &reference])
        .current_dir(repo_root)
        .output()?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1) {
        return Ok(false);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(GitError::CommandFailed(stderr))
}

/// Check whether the worktree has uncommitted changes or untracked files.
pub fn worktree_has_changes(worktree_path: &Path) -> Result<bool, GitError> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GitError::CommandFailed(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(!stdout.trim().is_empty())
}

/// Parse `git worktree list --porcelain` for active worktree paths and branch names.
///
/// Returns `(canonicalized_paths, branch_names)`. Used to detect directories
/// not tracked by git during stale worktree cleanup.
pub(crate) fn parse_worktree_list(repo_root: &Path) -> (HashSet<PathBuf>, HashSet<String>) {
    let Ok(output) = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_root)
        .output()
    else {
        return Default::default();
    };
    if !output.status.success() {
        return Default::default();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut paths = HashSet::new();
    let mut branches = HashSet::new();
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            let raw = PathBuf::from(p);
            // Canonicalize for symlink resolution (macOS /tmp → /private/tmp).
            // Fall back to the raw path so active worktrees are never missed.
            let resolved = raw.canonicalize().unwrap_or(raw);
            paths.insert(resolved);
        } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
            branches.insert(b.to_string());
        }
    }
    (paths, branches)
}

/// Reject names that could escape `.loopal/worktrees/` or inject into git commands.
pub(crate) fn validate_name(name: &str) -> Result<(), GitError> {
    let invalid = name.is_empty()
        || name.len() > 200
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.starts_with('.')
        || name.starts_with('-')
        || name.contains('\0')
        || name.contains(' ');
    if invalid {
        return Err(GitError::InvalidName(name.to_string()));
    }
    Ok(())
}
