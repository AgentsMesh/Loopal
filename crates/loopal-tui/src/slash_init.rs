use std::fs;
use std::path::Path;

use crate::app::App;

const LOOPAL_MD_TEMPLATE: &str = "\
# LOOPAL.md

This file provides guidance to Loopal when working with code in this repository.

## Build & Test Commands

```bash
# Add your build/test commands here
```

## Architecture

Describe your project architecture here.

## Code Conventions

Describe your coding conventions here.
";

const MEMORY_MD_TEMPLATE: &str = "\
# Project Memory

This file is managed by Loopal to remember key facts about the project.
";

/// Run the `/init` command — create project config scaffolding.
pub(crate) fn run_init(app: &mut App) {
    let cwd = &app.cwd;
    let mut created: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    // 1. LOOPAL.md at project root
    let instructions_path = cwd.join("LOOPAL.md");
    write_template(
        &instructions_path,
        LOOPAL_MD_TEMPLATE,
        &mut created,
        &mut skipped,
    );

    // 2. .loopal/ directory
    let dot_dir = cwd.join(".loopal");
    ensure_dir(&dot_dir, &mut created, &mut skipped);

    // 3. .loopal/memory/MEMORY.md
    let memory_dir = dot_dir.join("memory");
    ensure_dir(&memory_dir, &mut created, &mut skipped);
    let memory_path = memory_dir.join("MEMORY.md");
    write_template(&memory_path, MEMORY_MD_TEMPLATE, &mut created, &mut skipped);

    // Build summary message
    let mut lines = vec!["Initialized project:".to_string()];
    for item in &created {
        lines.push(format!("  ✓ Created {item}"));
    }
    for item in &skipped {
        lines.push(format!("  · {item} already exists"));
    }
    if created.is_empty() {
        lines.push("  (nothing to create — all files already exist)".to_string());
    }
    app.session.push_system_message(lines.join("\n"));
}

/// Write a template file if it doesn't exist yet.
fn write_template(
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
fn ensure_dir(path: &Path, created: &mut Vec<String>, skipped: &mut Vec<String>) {
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
/// Falls back to the full path if it doesn't contain recognizable segments.
fn display_relative(path: &Path) -> String {
    let s = path.to_string_lossy();
    // Find the last occurrence of a project-root marker pattern
    if let Some(pos) = s.rfind("/.loopal/") {
        let root_end = pos + 1; // skip the '/'
        return s[root_end..].to_string();
    }
    // For LOOPAL.md at project root
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_display_relative_loopal_md() {
        let p = PathBuf::from("/home/user/project/LOOPAL.md");
        assert_eq!(display_relative(&p), "LOOPAL.md");
    }

    #[test]
    fn test_display_relative_dotdir() {
        let p = PathBuf::from("/home/user/project/.loopal/memory/MEMORY.md");
        assert_eq!(display_relative(&p), ".loopal/memory/MEMORY.md");
    }

    #[test]
    fn test_write_template_creates_file() {
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
    fn test_write_template_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("TEST.md");
        fs::write(&file, "existing").unwrap();
        let mut created = Vec::new();
        let mut skipped = Vec::new();
        write_template(&file, "new content", &mut created, &mut skipped);
        assert!(created.is_empty());
        assert_eq!(skipped.len(), 1);
        // Content unchanged
        assert_eq!(fs::read_to_string(&file).unwrap(), "existing");
    }

    #[test]
    fn test_ensure_dir_creates() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        let mut created = Vec::new();
        let mut skipped = Vec::new();
        ensure_dir(&sub, &mut created, &mut skipped);
        assert_eq!(created.len(), 1);
        assert!(sub.is_dir());
    }

    #[test]
    fn test_ensure_dir_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        let mut created = Vec::new();
        let mut skipped = Vec::new();
        ensure_dir(dir.path(), &mut created, &mut skipped);
        assert!(created.is_empty());
        assert_eq!(skipped.len(), 1);
    }
}
