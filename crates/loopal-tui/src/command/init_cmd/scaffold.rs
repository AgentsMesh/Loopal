//! Filesystem scaffolding helpers for `/init`.

use std::fs;
use std::path::Path;

pub(super) const MEMORY_MD_TEMPLATE: &str = "\
# Project Memory

This file is managed by Loopal to remember key facts about the project.
";

/// Write a template file if it doesn't exist yet.
pub(super) fn write_template(
    path: &Path,
    content: &str,
    created: &mut Vec<String>,
    skipped: &mut Vec<String>,
) {
    let display = display_relative(path);
    if path.exists() {
        skipped.push(display);
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(path, content) {
        Ok(()) => created.push(display),
        Err(e) => created.push(format!("{display} (error: {e})")),
    }
}

/// Ensure a directory exists, tracking whether it was created or already existed.
pub(super) fn ensure_dir(path: &Path, created: &mut Vec<String>, skipped: &mut Vec<String>) {
    let display = format!("{}/", display_relative(path));
    if path.is_dir() {
        skipped.push(display);
        return;
    }
    match fs::create_dir_all(path) {
        Ok(()) => created.push(display),
        Err(e) => created.push(format!("{display} (error: {e})")),
    }
}

/// Extract a short relative-style display name from an absolute path.
pub(super) fn display_relative(path: &Path) -> String {
    let s = path.to_string_lossy();
    if let Some(pos) = s.rfind("/.loopal/") {
        let root_end = pos + 1;
        return s[root_end..].to_string();
    }
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn display_relative_loopal_md() {
        let p = PathBuf::from("/home/user/project/LOOPAL.md");
        assert_eq!(display_relative(&p), "LOOPAL.md");
    }

    #[test]
    fn display_relative_dotdir() {
        let p = PathBuf::from("/home/user/project/.loopal/memory/MEMORY.md");
        assert_eq!(display_relative(&p), ".loopal/memory/MEMORY.md");
    }

    #[test]
    fn write_template_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("TEST.md");
        let mut created = Vec::new();
        let mut skipped = Vec::new();
        write_template(&file, "hello", &mut created, &mut skipped);
        assert_eq!(created.len(), 1);
        assert!(skipped.is_empty());
        assert_eq!(fs::read_to_string(&file).unwrap(), "hello");
    }

    #[test]
    fn write_template_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("TEST.md");
        fs::write(&file, "existing").unwrap();
        let mut created = Vec::new();
        let mut skipped = Vec::new();
        write_template(&file, "new content", &mut created, &mut skipped);
        assert!(created.is_empty());
        assert_eq!(skipped.len(), 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), "existing");
    }

    #[test]
    fn ensure_dir_creates() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        let mut created = Vec::new();
        let mut skipped = Vec::new();
        ensure_dir(&sub, &mut created, &mut skipped);
        assert_eq!(created.len(), 1);
        assert!(sub.is_dir());
    }

    #[test]
    fn ensure_dir_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        let mut created = Vec::new();
        let mut skipped = Vec::new();
        ensure_dir(dir.path(), &mut created, &mut skipped);
        assert!(created.is_empty());
        assert_eq!(skipped.len(), 1);
    }
}
