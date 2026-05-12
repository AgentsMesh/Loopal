use std::fs;
use std::io::Write;
use std::path::Path;

const GITIGNORE: &str = "\
# Auto-managed by Loopal. Do not edit.
worktrees/
plans/
settings.local.json
LOOPAL.local.md
";

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
