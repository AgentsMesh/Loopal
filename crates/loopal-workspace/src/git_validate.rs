use crate::WorkspaceError;

pub(crate) fn validate_worktree_name(name: &str) -> Result<(), WorkspaceError> {
    let mut chars = name.chars();
    let valid = name.len() <= 64
        && chars.next().is_some_and(|ch| ch.is_ascii_alphanumeric())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(WorkspaceError::invalid("invalid worktree name"))
    }
}
