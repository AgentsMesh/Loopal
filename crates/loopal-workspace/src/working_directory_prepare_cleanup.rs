use std::path::Path;

use crate::WorkspaceError;

pub(crate) fn cleanup_failed_prepare(
    repo: &Path,
    info: &loopal_git::WorktreeInfo,
    error: WorkspaceError,
) -> WorkspaceError {
    if loopal_git::cleanup_if_clean(repo, info) {
        return error;
    }
    WorkspaceError::new(
        "worktree_retained",
        format!(
            "{}; worktree retained at {} on branch {} because safe cleanup could not be proven",
            error.message,
            info.path.display(),
            info.branch,
        ),
    )
}
