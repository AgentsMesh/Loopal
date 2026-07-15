use crate::git_command::run_git;
use crate::types::WorkspacePathParams;
use crate::{WorkspaceError, WorkspaceService};

impl WorkspaceService {
    pub async fn git_stage(&self, input: WorkspacePathParams) -> Result<(), WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        let path = self.guard.resolve(&input.path, true)?;
        let relative = self.guard.relative(&path)?;
        let _lock = self.write_lock.lock().await;
        run_git(
            self.guard.root().to_path_buf(),
            vec!["add".into(), "--".into(), relative],
        )
        .await?;
        self.publish_git_changed();
        Ok(())
    }

    pub async fn git_unstage(&self, input: WorkspacePathParams) -> Result<(), WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        let path = self.guard.resolve(&input.path, true)?;
        let relative = self.guard.relative(&path)?;
        let _lock = self.write_lock.lock().await;
        let reset = run_git(
            self.guard.root().to_path_buf(),
            vec!["reset".into(), "-q".into(), "--".into(), relative],
        )
        .await;
        if reset.is_err() {
            let relative = self.guard.relative(&path)?;
            run_git(
                self.guard.root().to_path_buf(),
                vec![
                    "rm".into(),
                    "--cached".into(),
                    "-q".into(),
                    "--".into(),
                    relative,
                ],
            )
            .await?;
        }
        self.publish_git_changed();
        Ok(())
    }
}
