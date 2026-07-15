use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::GitError;

const GITIGNORE: &str = "\
# Auto-managed by Loopal. Do not edit.
worktrees/
plans/
settings.local.json
LOOPAL.local.md
";
const WORKTREE_EXCLUDE: &str = "/.loopal/worktrees/";

pub(crate) fn ensure_worktree_exclude(repo_root: &Path) -> Result<(), GitError> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", "info/exclude"])
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        return Err(GitError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return Err(GitError::CommandFailed(
            "git returned an empty exclude path".into(),
        ));
    }
    let path = PathBuf::from(raw);
    let path = if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    };
    let existing = match fs::read(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    if String::from_utf8_lossy(&existing)
        .lines()
        .any(|line| line.trim() == WORKTREE_EXCLUDE)
    {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    if !existing.is_empty() && !existing.ends_with(b"\n") {
        file.write_all(b"\n")?;
    }
    writeln!(file, "{WORKTREE_EXCLUDE}")?;
    file.sync_all()?;
    Ok(())
}

pub fn ensure_loopal_gitignore(loopal_dir: &Path) {
    if !is_in_git_worktree(loopal_dir) {
        return;
    }
    let path = loopal_dir.join(".gitignore");
    if matches!(fs::read_to_string(&path), Ok(s) if s == GITIGNORE) {
        return;
    }
    let _ = atomic_write(&path, GITIGNORE);
}

fn is_in_git_worktree(start: &Path) -> bool {
    let mut cur: &Path = start;
    loop {
        if cur.join(".git").exists() {
            return true;
        }
        match cur.parent() {
            Some(p) if p != cur => cur = p,
            _ => return false,
        }
    }
}

fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let tmp = path.with_extension("gitignore.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}
