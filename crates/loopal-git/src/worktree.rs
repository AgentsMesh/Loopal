use std::collections::HashSet;
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

    // Only delete orphaned branches — skip if the branch is used by an active worktree.
    let (_, active_branches) = parse_worktree_list(repo_root);
    if !active_branches.contains(&branch) {
        let _ = Command::new("git")
            .args(["branch", "-D", &branch])
            .current_dir(repo_root)
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status();
    }

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

    Ok(WorktreeInfo {
        path: wt_dir,
        branch,
        name: name.to_string(),
    })
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

/// Remove a worktree if it has no uncommitted changes. Returns `true` if removed.
/// Canonical cleanup logic shared by bootstrap, agent spawn, and spawn-failure paths.
pub fn cleanup_if_clean(repo_root: &Path, info: &WorktreeInfo) -> bool {
    let has_changes = worktree_has_changes(&info.path).unwrap_or(true);
    if !has_changes {
        let _ = remove_worktree(repo_root, &info.name, false);
        return true;
    }
    false
}

/// Parse `git worktree list --porcelain` for active worktree paths and branch names.
///
/// Returns `(canonicalized_paths, branch_names)`. Used by `create_worktree`
/// (to avoid deleting branches in use) and `cleanup_stale_worktrees`
/// (to detect directories not tracked by git).
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
fn validate_name(name: &str) -> Result<(), GitError> {
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

/// Append `.loopal/worktrees/` to `.gitignore` if not already present.
fn ensure_gitignore_entry(repo_root: &Path) {
    let gitignore = repo_root.join(".gitignore");
    let entry = ".loopal/worktrees/";

    let content = std::fs::read_to_string(&gitignore).unwrap_or_default();
    if content.lines().any(|line| line.trim() == entry) {
        return;
    }

    // Append-only write to avoid TOCTOU overwrite of concurrent modifications
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore)
    {
        let prefix = if content.is_empty() || content.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        let _ = writeln!(file, "{prefix}{entry}");
    }
}
