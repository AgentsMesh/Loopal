use std::path::{Component, Path, PathBuf};

use crate::WorkspaceError;

#[derive(Clone)]
pub struct RootGuard {
    root: PathBuf,
}

impl RootGuard {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let root = canonicalize(root.as_ref())?;
        if !root.is_dir() {
            return Err(WorkspaceError::invalid("workspace root is not a directory"));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve(&self, raw: &str, allow_missing: bool) -> Result<PathBuf, WorkspaceError> {
        if raw.len() > 4_096 || raw.contains('\0') {
            return Err(WorkspaceError::invalid("path is invalid or too long"));
        }
        let relative = Path::new(raw);
        if relative.is_absolute() {
            return Err(WorkspaceError::new(
                "path_outside_root",
                "absolute path denied",
            ));
        }
        if relative.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(WorkspaceError::new(
                "path_outside_root",
                "parent traversal denied",
            ));
        }
        let joined = self.root.join(relative);
        let resolved = if allow_missing {
            self.resolve_missing(&joined)?
        } else {
            canonicalize(&joined)?
        };
        if !resolved.starts_with(&self.root) {
            return Err(WorkspaceError::new(
                "path_outside_root",
                format!("path escapes workspace: {}", resolved.display()),
            ));
        }
        Ok(resolved)
    }

    pub fn relative(&self, path: &Path) -> Result<String, WorkspaceError> {
        let rel = path.strip_prefix(&self.root).map_err(WorkspaceError::io)?;
        Ok(rel.to_string_lossy().replace('\\', "/"))
    }

    fn resolve_missing(&self, path: &Path) -> Result<PathBuf, WorkspaceError> {
        let mut missing = Vec::new();
        let mut cursor = path;
        loop {
            match cursor.canonicalize() {
                Ok(resolved) => {
                    let mut resolved = loopal_backend::path::strip_win_prefix(resolved);
                    for part in missing.iter().rev() {
                        resolved.push(part);
                    }
                    return Ok(resolved);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let name = cursor.file_name().ok_or_else(|| {
                        WorkspaceError::new("path_outside_root", "path has no existing ancestor")
                    })?;
                    missing.push(name.to_os_string());
                    cursor = cursor.parent().ok_or_else(|| {
                        WorkspaceError::new("path_outside_root", "path has no parent")
                    })?;
                }
                Err(error) => return Err(WorkspaceError::io(error)),
            }
        }
    }
}

pub(crate) fn canonicalize(path: &Path) -> Result<PathBuf, WorkspaceError> {
    path.canonicalize()
        .map(loopal_backend::path::strip_win_prefix)
        .map_err(WorkspaceError::io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_and_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let guard = RootGuard::new(dir.path()).unwrap();
        assert_eq!(
            guard.resolve("../nope", true).unwrap_err().code,
            "path_outside_root"
        );
        assert_eq!(
            guard.resolve("/tmp/nope", true).unwrap_err().code,
            "path_outside_root"
        );
    }
}
