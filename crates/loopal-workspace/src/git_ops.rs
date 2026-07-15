use std::path::{Path, PathBuf};

use crate::git_command::read_git;
use crate::git_parse;
use crate::git_types::{CreateWorktreeParams, GitStatus, RemoveWorktreeParams, Worktree};
use crate::git_validate::validate_worktree_name;
use crate::types::WorkspaceParams;
use crate::{WorkspaceError, WorkspaceService};

impl WorkspaceService {
    pub async fn git_status(&self, input: WorkspaceParams) -> Result<GitStatus, WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        let output = read_git(
            self.guard.root().to_path_buf(),
            vec![
                "status".into(),
                "--porcelain=v1".into(),
                "-z".into(),
                "--branch".into(),
                "--untracked-files=all".into(),
                "--".into(),
                ".".into(),
            ],
        )
        .await?;
        Ok(git_parse::status(&output.stdout))
    }

    pub async fn list_worktrees(
        &self,
        input: WorkspaceParams,
    ) -> Result<Vec<Worktree>, WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        let repo = self.repo_root()?;
        let output = read_git(
            repo.clone(),
            vec!["worktree".into(), "list".into(), "--porcelain".into()],
        )
        .await?;
        let mut result = Vec::new();
        for raw in git_parse::worktrees(&String::from_utf8_lossy(&output.stdout)) {
            let path = PathBuf::from(&raw.path);
            let is_main = path.canonicalize().ok().as_ref() == Some(&repo);
            let has_changes = read_git(path.clone(), vec!["status".into(), "--porcelain".into()])
                .await
                .map(|value| !value.stdout.is_empty())
                .unwrap_or(true);
            let id = if is_main {
                self.workspace_id.clone()
            } else {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            };
            result.push(Worktree {
                id,
                path: raw.path,
                branch: raw.branch,
                head: raw.head.unwrap_or_default(),
                is_main,
                has_changes,
            });
        }
        Ok(result)
    }

    pub async fn create_worktree(
        &self,
        input: CreateWorktreeParams,
    ) -> Result<Worktree, WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        validate_worktree_name(&input.name)?;
        let _lock = self.write_lock.lock().await;
        let repo = self.repo_root()?;
        let info =
            tokio::task::spawn_blocking(move || loopal_git::create_worktree(&repo, &input.name))
                .await
                .map_err(WorkspaceError::io)?
                .map_err(WorkspaceError::io)?;
        self.publish_git_changed();
        Ok(Worktree {
            id: info.name,
            path: info.path.to_string_lossy().into_owned(),
            branch: Some(info.branch),
            head: git_head(&info.path).await?,
            is_main: false,
            has_changes: false,
        })
    }

    pub async fn remove_worktree(&self, input: RemoveWorktreeParams) -> Result<(), WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        validate_worktree_name(&input.name)?;
        let _lock = self.write_lock.lock().await;
        let repo = self.repo_root()?;
        tokio::task::spawn_blocking(move || {
            loopal_git::remove_worktree(&repo, &input.name, input.force)
        })
        .await
        .map_err(WorkspaceError::io)?
        .map_err(WorkspaceError::io)?;
        self.publish_git_changed();
        Ok(())
    }

    pub(crate) fn repo_root(&self) -> Result<PathBuf, WorkspaceError> {
        let root = self.guard.root();
        let repo = loopal_git::repo_root(root)
            .ok_or_else(|| WorkspaceError::new("not_git_repository", "workspace is not in git"))?;
        let repo = repo.canonicalize().map_err(WorkspaceError::io)?;
        (repo == root).then_some(repo).ok_or_else(|| {
            WorkspaceError::new(
                "git_root_outside_workspace",
                "workspace root must equal repository root",
            )
        })
    }
}

async fn git_optional(cwd: &Path, args: &[&str]) -> Result<String, WorkspaceError> {
    match read_git(
        cwd.to_path_buf(),
        args.iter().map(|arg| (*arg).into()).collect(),
    )
    .await
    {
        Ok(value) => Ok(String::from_utf8_lossy(&value.stdout).into_owned()),
        Err(error) if error.code == "git_error" => Ok(String::new()),
        Err(error) => Err(error),
    }
}

async fn git_head(cwd: &Path) -> Result<String, WorkspaceError> {
    git_optional(cwd, &["rev-parse", "HEAD"])
        .await
        .map(|value| value.trim().to_string())
}
