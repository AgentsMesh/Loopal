use std::path::Path;

use tokio::io::AsyncReadExt;

use crate::git_command::{MAX_GIT_RESPONSE_BYTES, read_git};
use crate::git_types::GitDiff;
use crate::types::WorkspacePathParams;
use crate::{WorkspaceError, WorkspaceService};

impl WorkspaceService {
    pub async fn git_diff(&self, input: WorkspacePathParams) -> Result<GitDiff, WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        let path = self.guard.resolve(&input.path, true)?;
        let relative = self.guard.relative(&path)?;
        let root = self.guard.root();
        let has_head = !optional_git(root, &["rev-parse", "--verify", "HEAD"])
            .await?
            .is_empty();
        let mut args = vec!["diff".into(), "--no-ext-diff".into(), "--no-color".into()];
        if has_head {
            args.push("HEAD".into());
        }
        args.extend(["--".into(), relative.clone()]);
        let mut patch = text(read_git(root.to_path_buf(), args).await?.stdout);
        let original = text(optional_git(root, &["show", &format!("HEAD:{relative}")]).await?);
        let modified = read_modified(&path).await?;
        if patch.is_empty() && original.is_empty() && !modified.is_empty() {
            patch = format!(
                "--- /dev/null\n+++ b/{relative}\n@@ -0,0 +1 @@\n+{}\n",
                modified.replace('\n', "\n+")
            );
        }
        require_response_limit(&patch, &original, &modified)?;
        Ok(GitDiff {
            path: relative,
            patch,
            original,
            modified,
        })
    }
}

async fn optional_git(cwd: &Path, args: &[&str]) -> Result<Vec<u8>, WorkspaceError> {
    match read_git(
        cwd.to_path_buf(),
        args.iter().map(|arg| (*arg).into()).collect(),
    )
    .await
    {
        Ok(value) => Ok(value.stdout),
        Err(error) if error.code == "git_error" => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

async fn read_modified(path: &Path) -> Result<String, WorkspaceError> {
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_GIT_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > MAX_GIT_RESPONSE_BYTES {
        return Err(too_large());
    }
    Ok(text(bytes))
}

fn require_response_limit(
    parts: &str,
    original: &str,
    modified: &str,
) -> Result<(), WorkspaceError> {
    let size = parts
        .len()
        .saturating_add(original.len())
        .saturating_add(modified.len());
    (size <= MAX_GIT_RESPONSE_BYTES)
        .then_some(())
        .ok_or_else(too_large)
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

fn too_large() -> WorkspaceError {
    WorkspaceError::new("response_too_large", "git diff exceeds 8 MiB")
}
