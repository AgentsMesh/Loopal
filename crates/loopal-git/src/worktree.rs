use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::GitError;

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
    validate_name(name)?;

    let wt_dir = repo_root.join(".loopal").join("worktrees").join(name);
    if wt_dir.exists() {
        return Err(GitError::WorktreeExists(name.to_string()));
    }

    let branch = format!("loopal-wt-{name}");

    // If the branch already exists (orphaned from a previous run), delete it first
    let _ = Command::new("git")
        .args(["branch", "-D", &branch])
        .current_dir(repo_root)
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .status();

    if let Some(parent) = wt_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }

    ensure_gitignore_entry(repo_root);

    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch])
        .arg(&wt_dir)
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GitError::CommandFailed(stderr));
    }

    Ok(WorktreeInfo { path: wt_dir, branch, name: name.to_string() })
}

/// Remove a worktree by name.
///
/// If `force` is true, uses `--force` to remove even with uncommitted changes.
pub fn remove_worktree(repo_root: &Path, name: &str, force: bool) -> Result<(), GitError> {
    validate_name(name)?;

    let wt_dir = repo_root.join(".loopal").join("worktrees").join(name);
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    let wt_str = wt_dir.to_string_lossy().to_string();
    args.push(&wt_str);

    let output = Command::new("git")
        .args(&args)
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GitError::CommandFailed(stderr));
    }

    // Also delete the branch
    let branch = format!("loopal-wt-{name}");
    let _ = Command::new("git")
        .args(["branch", "-D", &branch])
        .current_dir(repo_root)
        .output();

    Ok(())
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

/// Remove stale worktrees from `.loopal/worktrees/` (best-effort).
///
/// A worktree is considered stale if `git worktree list` no longer references it
/// (e.g. directory was partially deleted) or the directory is empty.
/// Called at startup to prevent orphan accumulation.
pub fn cleanup_stale_worktrees(repo_root: &Path) {
    let wt_base = repo_root.join(".loopal").join("worktrees");
    let Ok(entries) = std::fs::read_dir(&wt_base) else { return };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };

        // If the directory is not a valid git worktree, clean it up
        let is_valid = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !is_valid {
            let _ = remove_worktree(repo_root, name, true);
        }
    }
}

/// Reject names that could escape `.loopal/worktrees/` or inject into git commands.
fn validate_name(name: &str) -> Result<(), GitError> {
    let invalid = name.is_empty()
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

/// Append `.loopal/worktrees/` to `.gitignore` if not already present.
fn ensure_gitignore_entry(repo_root: &Path) {
    let gitignore = repo_root.join(".gitignore");
    let entry = ".loopal/worktrees/";

    let content = std::fs::read_to_string(&gitignore).unwrap_or_default();
    if content.lines().any(|line| line.trim() == entry) {
        return;
    }

    // Append-only write to avoid TOCTOU overwrite of concurrent modifications
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&gitignore) {
        let prefix = if content.is_empty() || content.ends_with('\n') { "" } else { "\n" };
        let _ = writeln!(file, "{prefix}{entry}");
    }
}
