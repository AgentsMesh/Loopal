use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::WorkspaceError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingDirectoryInfo {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<WorkingDirectoryGit>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingDirectoryGit {
    pub root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    pub dirty: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedWorkingDirectory {
    pub path: String,
    pub branch: String,
    pub name: String,
}

pub fn inspect_working_directory(path: &Path) -> Result<WorkingDirectoryInfo, WorkspaceError> {
    let path = canonical_directory(path)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            WorkspaceError::new("unsafe_working_directory", "filesystem roots are denied")
        })?
        .to_string();
    let git = loopal_git::repo_root(&path)
        .and_then(|root| root.canonicalize().ok())
        .map(|root| WorkingDirectoryGit {
            root: root.to_string_lossy().into_owned(),
            branch: loopal_git::current_branch(&path),
            head: current_head(&path),
            dirty: loopal_git::worktree_has_changes(&path).unwrap_or(true),
        });
    Ok(WorkingDirectoryInfo {
        path: path.to_string_lossy().into_owned(),
        name,
        git,
    })
}

pub fn prepare_worktree_directory(
    path: &Path,
    name: &str,
    expected_root: &Path,
    expected_head: Option<&str>,
) -> Result<PreparedWorkingDirectory, WorkspaceError> {
    validate_name(name)?;
    let selected = canonical_directory(path)?;
    if selected != path {
        return Err(changed());
    }
    let repo = loopal_git::repo_root(&selected)
        .ok_or_else(|| {
            WorkspaceError::new("not_git_repository", "working directory is not in Git")
        })?
        .canonicalize()
        .map_err(WorkspaceError::io)?;
    let expected_head = expected_head.ok_or_else(|| {
        WorkspaceError::new(
            "worktree_unborn_head",
            "Git needs an initial commit before creating a worktree",
        )
    })?;
    if repo != expected_root || current_head(&selected).as_deref() != Some(expected_head) {
        return Err(changed());
    }
    let relative = selected.strip_prefix(&repo).map_err(|_| {
        WorkspaceError::new(
            "git_root_mismatch",
            "Git root does not contain working directory",
        )
    })?;
    let info = loopal_git::create_worktree_at(&repo, name, expected_head).map_err(git_error)?;
    let path = info.path.join(relative);
    if !path.is_dir() {
        return Err(
            crate::working_directory_prepare_cleanup::cleanup_failed_prepare(
                &repo,
                &info,
                WorkspaceError::new(
                    "worktree_directory_missing",
                    "selected subdirectory is absent in the new worktree",
                ),
            ),
        );
    }
    let path = path.canonicalize().map_err(|error| {
        crate::working_directory_prepare_cleanup::cleanup_failed_prepare(
            &repo,
            &info,
            WorkspaceError::io(error),
        )
    })?;
    Ok(PreparedWorkingDirectory {
        path: path.to_string_lossy().into_owned(),
        branch: info.branch,
        name: info.name,
    })
}

pub(crate) fn canonical_directory(path: &Path) -> Result<PathBuf, WorkspaceError> {
    if !path.is_absolute() || path.as_os_str().len() > 4_096 {
        return Err(WorkspaceError::new(
            "invalid_working_directory",
            "absolute directory required",
        ));
    }
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(WorkspaceError::new(
            "invalid_working_directory",
            "parent traversal denied",
        ));
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| WorkspaceError::new("working_directory_unavailable", error.to_string()))?;
    if !canonical.is_dir() {
        return Err(WorkspaceError::new(
            "working_directory_unavailable",
            "path is not a directory",
        ));
    }
    Ok(canonical)
}

pub(crate) fn validate_name(name: &str) -> Result<(), WorkspaceError> {
    let mut chars = name.chars();
    let valid = name.len() <= 64
        && chars.next().is_some_and(|ch| ch.is_ascii_alphanumeric())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'));
    valid.then_some(()).ok_or_else(|| {
        WorkspaceError::new(
            "invalid_worktree_name",
            "worktree name must use letters, digits, _ or -",
        )
    })
}

pub(crate) fn git_error(error: loopal_git::GitError) -> WorkspaceError {
    let code = match error {
        loopal_git::GitError::WorktreeExists(_) => "worktree_exists",
        loopal_git::GitError::InvalidName(_) => "invalid_worktree_name",
        loopal_git::GitError::NotARepo => "not_git_repository",
        loopal_git::GitError::GitNotFound => "git_unavailable",
        loopal_git::GitError::CommandFailed(_) => "git_command_failed",
        loopal_git::GitError::Io(_) => "io_error",
    };
    WorkspaceError::new(code, error.to_string())
}

fn current_head(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(path)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_ascii_lowercase()
        })
        .filter(|value| !value.is_empty())
}

fn changed() -> WorkspaceError {
    WorkspaceError::new(
        "working_directory_changed",
        "working directory, Git repository, or HEAD changed; select it again",
    )
}
