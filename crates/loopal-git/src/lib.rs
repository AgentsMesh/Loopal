pub mod repo;
pub mod worktree;

pub use repo::{current_branch, is_git_repo, repo_root};
pub use worktree::{
    WorktreeInfo, cleanup_stale_worktrees, create_worktree,
    remove_worktree, worktree_has_changes,
};

use std::fmt;

/// Errors from git operations.
#[derive(Debug)]
pub enum GitError {
    /// The path is not inside a git repository.
    NotARepo,
    /// A worktree with this name already exists.
    WorktreeExists(String),
    /// The worktree name contains invalid characters.
    InvalidName(String),
    /// The git command failed with stderr output.
    CommandFailed(String),
    /// The `git` binary was not found on `$PATH`.
    GitNotFound,
    /// I/O error (e.g. creating directories).
    Io(std::io::Error),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotARepo => write!(f, "not a git repository"),
            Self::WorktreeExists(name) => write!(f, "worktree '{name}' already exists"),
            Self::InvalidName(name) => write!(f, "invalid worktree name: '{name}'"),
            Self::CommandFailed(msg) => write!(f, "git command failed: {msg}"),
            Self::GitNotFound => write!(f, "git not found — is git installed?"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for GitError {}

impl From<std::io::Error> for GitError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::NotFound {
            Self::GitNotFound
        } else {
            Self::Io(e)
        }
    }
}
