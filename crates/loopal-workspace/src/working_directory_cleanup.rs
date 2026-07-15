use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::WorkspaceError;
use crate::working_directory::{canonical_directory, validate_name};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanedWorkingDirectory {
    pub path: String,
    pub removed: bool,
}

pub fn cleanup_prepared_worktree(
    selected_path: &Path,
    name: &str,
    expected_path: &Path,
) -> Result<CleanedWorkingDirectory, WorkspaceError> {
    validate_name(name)?;
    let selected = canonical_directory(selected_path)?;
    let repo = loopal_git::repo_root(&selected)
        .ok_or_else(|| {
            WorkspaceError::new("not_git_repository", "working directory is not in Git")
        })?
        .canonicalize()
        .map_err(WorkspaceError::io)?;
    let relative = selected.strip_prefix(&repo).map_err(|_| {
        WorkspaceError::new(
            "git_root_mismatch",
            "Git root does not contain working directory",
        )
    })?;
    let worktree = repo.join(".loopal").join("worktrees").join(name);
    let prepared = worktree.join(relative);
    if prepared != expected_path || canonical_exact(&worktree).as_ref() != Some(&worktree) {
        return Err(retained(
            "worktree_cleanup_unsafe",
            &prepared,
            "prepared worktree identity changed; it was retained",
        ));
    }
    let info = loopal_git::WorktreeInfo {
        path: worktree,
        branch: format!("loopal-wt-{name}"),
        name: name.to_string(),
    };
    if !loopal_git::cleanup_if_clean(&repo, &info) {
        return Err(retained(
            "worktree_cleanup_unsafe",
            &prepared,
            "prepared worktree was not proven safe to remove; it was retained",
        ));
    }
    if info.path.exists() {
        return Err(retained(
            "worktree_cleanup_failed",
            &prepared,
            "Git cleanup returned without removing the worktree; it was retained",
        ));
    }
    Ok(CleanedWorkingDirectory {
        path: prepared.to_string_lossy().into_owned(),
        removed: true,
    })
}

fn canonical_exact(path: &Path) -> Option<PathBuf> {
    path.canonicalize().ok()
}

fn retained(code: &'static str, path: &Path, reason: &str) -> WorkspaceError {
    WorkspaceError::new(code, format!("{}: {reason}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;
    use crate::{inspect_working_directory, prepare_worktree_directory};

    #[test]
    fn removes_only_the_exact_clean_prepared_worktree() {
        let root = repository();
        let inspected = inspect_working_directory(root.path()).unwrap();
        let git = inspected.git.unwrap();
        let prepared = prepare_worktree_directory(
            Path::new(&inspected.path),
            "clean",
            Path::new(&git.root),
            git.head.as_deref(),
        )
        .unwrap();
        let cleaned =
            cleanup_prepared_worktree(root.path(), "clean", Path::new(&prepared.path)).unwrap();
        assert!(cleaned.removed);
        assert!(!Path::new(&prepared.path).exists());
    }

    #[test]
    fn retains_dirty_or_mismatched_worktrees() {
        let root = repository();
        let inspected = inspect_working_directory(root.path()).unwrap();
        let git = inspected.git.unwrap();
        let prepared = prepare_worktree_directory(
            Path::new(&inspected.path),
            "retained",
            Path::new(&git.root),
            git.head.as_deref(),
        )
        .unwrap();
        assert_eq!(
            cleanup_prepared_worktree(root.path(), "retained", &root.path().join("wrong"),)
                .unwrap_err()
                .code,
            "worktree_cleanup_unsafe"
        );
        std::fs::write(Path::new(&prepared.path).join("new.txt"), "changed\n").unwrap();
        assert_eq!(
            cleanup_prepared_worktree(root.path(), "retained", Path::new(&prepared.path),)
                .unwrap_err()
                .code,
            "worktree_cleanup_unsafe"
        );
        assert!(Path::new(&prepared.path).is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_same_repo_symlink_replacement() {
        use std::os::unix::fs::symlink;

        let root = repository();
        let first = root.path().join("first");
        let second = root.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let inspected = inspect_working_directory(&first).unwrap();
        let git = inspected.git.unwrap();
        std::fs::rename(&first, root.path().join("moved")).unwrap();
        symlink(&second, &first).unwrap();

        assert_eq!(
            prepare_worktree_directory(
                Path::new(&inspected.path),
                "swapped",
                Path::new(&git.root),
                git.head.as_deref(),
            )
            .unwrap_err()
            .code,
            "working_directory_changed"
        );
    }

    fn repository() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        run(root.path(), &["init", "-b", "main"]);
        run(
            root.path(),
            &["config", "user.email", "desktop@example.invalid"],
        );
        run(root.path(), &["config", "user.name", "Loopal Desktop"]);
        std::fs::write(root.path().join("tracked.txt"), "tracked\n").unwrap();
        run(root.path(), &["add", "."]);
        run(root.path(), &["commit", "-m", "initial"]);
        root
    }

    fn run(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
