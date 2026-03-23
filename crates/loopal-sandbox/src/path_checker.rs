use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use loopal_config::{PathDecision, ResolvedPolicy, SandboxPolicy};

/// Check whether a path operation is allowed under the resolved sandbox policy.
///
/// `is_write` indicates whether the operation modifies the filesystem.
/// Returns `PathDecision::Allow` when the operation is permitted.
pub fn check_path(policy: &ResolvedPolicy, path: &Path, is_write: bool) -> PathDecision {
    if policy.policy == SandboxPolicy::Disabled {
        return PathDecision::Allow;
    }

    // Resolve symlinks and normalize to a canonical path
    let canonical = match resolve_canonical(path) {
        Ok(p) => p,
        Err(reason) => return PathDecision::DenyWrite(reason),
    };

    // Check read denials first (applies to both read and write)
    if let Some(reason) = check_deny_globs(&canonical, &policy.deny_read_globs, "read") {
        return PathDecision::DenyRead(reason);
    }

    if !is_write {
        return PathDecision::Allow;
    }

    // Read-only mode blocks all writes
    if policy.policy == SandboxPolicy::ReadOnly {
        return PathDecision::DenyWrite("read-only sandbox: all writes are blocked".into());
    }

    // Check explicit write denials
    if let Some(reason) = check_deny_globs(&canonical, &policy.deny_write_globs, "write") {
        return PathDecision::DenyWrite(reason);
    }

    // Check whether the path is under a writable directory
    if is_under_writable(&canonical, &policy.writable_paths) {
        return PathDecision::Allow;
    }

    PathDecision::DenyWrite(format!(
        "path outside writable directories: {}",
        canonical.display()
    ))
}

/// Resolve a path to its canonical form, detecting symlink escapes and `..` traversal.
fn resolve_canonical(path: &Path) -> Result<PathBuf, String> {
    // Try canonical resolution first (file exists)
    if let Ok(canonical) = path.canonicalize() {
        return Ok(canonical);
    }

    // Walk up ancestor chain to find the deepest existing directory, then
    // append the remaining relative suffix. This correctly resolves symlinks
    // (e.g. /tmp → /private/tmp on macOS) for paths that don't yet exist.
    let mut ancestors: Vec<&std::ffi::OsStr> = Vec::new();
    let mut current: &Path = path;
    loop {
        if let Ok(canon) = current.canonicalize() {
            let mut result = canon;
            for component in ancestors.iter().rev() {
                result = result.join(component);
            }
            return Ok(result);
        }
        match (current.file_name(), current.parent()) {
            (Some(name), Some(parent)) => {
                ancestors.push(name);
                current = parent;
            }
            _ => break,
        }
    }

    // Fallback: detect obvious `..` traversal
    let path_str = path.to_string_lossy();
    if path_str.contains("..") {
        return Err(format!("path contains '..': {path_str}"));
    }

    Ok(path.to_path_buf())
}

/// Check if a canonical path matches any deny glob patterns.
fn check_deny_globs(path: &Path, globs: &[String], operation: &str) -> Option<String> {
    let glob_set = build_glob_set(globs);
    let path_str = path.to_string_lossy();

    if glob_set.is_match(path_str.as_ref()) {
        return Some(format!(
            "{operation} denied by glob pattern: {}",
            path.display()
        ));
    }
    None
}

/// Check whether a path falls under any of the writable directories.
fn is_under_writable(path: &Path, writable_paths: &[PathBuf]) -> bool {
    writable_paths.iter().any(|wp| path.starts_with(wp))
}

/// Build a GlobSet from string patterns, skipping invalid patterns.
fn build_glob_set(patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}
