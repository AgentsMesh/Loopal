use std::path::Path;
use std::process::Command;

use crate::GitError;
use crate::worktree::{WorktreeInfo, validate_name, worktree_has_changes};

pub fn remove_worktree(repo_root: &Path, name: &str, force: bool) -> Result<(), GitError> {
    validate_name(name)?;
    let path = repo_root.join(".loopal").join("worktrees").join(name);
    let branch = format!("loopal-wt-{name}");
    if !force {
        require_merged(repo_root, &path, "HEAD", name)?;
        if branch_exists(repo_root, &branch)? {
            require_merged(repo_root, repo_root, &format!("refs/heads/{branch}"), name)?;
        }
    }

    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    let path_value = path.to_string_lossy().into_owned();
    args.push(&path_value);
    run(repo_root, &args)?;

    if branch_exists(repo_root, &branch)? {
        run(
            repo_root,
            &["branch", if force { "-D" } else { "-d" }, &branch],
        )?;
    }
    Ok(())
}

pub fn cleanup_if_clean(repo_root: &Path, info: &WorktreeInfo) -> bool {
    matches!(worktree_has_changes(&info.path), Ok(false))
        && remove_worktree(repo_root, &info.name, false).is_ok()
}

fn require_merged(
    repo_root: &Path,
    revision_cwd: &Path,
    revision: &str,
    name: &str,
) -> Result<(), GitError> {
    let commit = output(revision_cwd, &["rev-parse", "--verify", revision])?;
    let result = Command::new("git")
        .args(["merge-base", "--is-ancestor", &commit, "HEAD"])
        .current_dir(repo_root)
        .output()?;
    if result.status.success() {
        return Ok(());
    }
    if result.status.code() == Some(1) {
        return Err(GitError::CommandFailed(format!(
            "worktree '{name}' contains commits not merged into repository HEAD",
        )));
    }
    Err(command_error(result.stderr))
}

fn branch_exists(repo_root: &Path, branch: &str) -> Result<bool, GitError> {
    let reference = format!("refs/heads/{branch}");
    let result = Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &reference])
        .current_dir(repo_root)
        .output()?;
    match result.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(command_error(result.stderr)),
    }
}

fn output(cwd: &Path, args: &[&str]) -> Result<String, GitError> {
    let result = Command::new("git").args(args).current_dir(cwd).output()?;
    if !result.status.success() {
        return Err(command_error(result.stderr));
    }
    Ok(String::from_utf8_lossy(&result.stdout).trim().to_string())
}

fn run(cwd: &Path, args: &[&str]) -> Result<(), GitError> {
    let result = Command::new("git").args(args).current_dir(cwd).output()?;
    result
        .status
        .success()
        .then_some(())
        .ok_or_else(|| command_error(result.stderr))
}

fn command_error(stderr: Vec<u8>) -> GitError {
    GitError::CommandFailed(String::from_utf8_lossy(&stderr).trim().to_string())
}
