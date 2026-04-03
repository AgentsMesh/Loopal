//! Unified path resolution — single entry point for all path operations.

use std::path::{Path, PathBuf};

use loopal_config::{PathDecision, ResolvedPolicy};
use loopal_error::ToolIoError;

/// Resolve a user-supplied path to an absolute, canonicalized path.
///
/// When `policy` is present, delegates to the sandbox `check_path`.
/// When absent, only rejects relative paths that use `..` to escape cwd
/// (basic traversal guard). Absolute paths are always allowed — the sandbox
/// policy is responsible for enforcing broader path boundaries.
pub fn resolve(
    cwd: &Path,
    raw: &str,
    is_write: bool,
    policy: Option<&ResolvedPolicy>,
) -> Result<PathBuf, ToolIoError> {
    let path = to_absolute(cwd, raw);

    if let Some(pol) = policy {
        return check_with_policy(pol, &path, is_write);
    }

    // No sandbox policy — reject relative `..` traversal escaping cwd
    if !Path::new(raw).is_absolute() {
        let canonical = resolve_canonical(&path)?;
        if !canonical.starts_with(cwd) {
            return Err(ToolIoError::PathDenied(format!(
                "path escapes working directory: {}",
                canonical.display()
            )));
        }
        return Ok(canonical);
    }

    // Absolute paths: resolve canonical without write restrictions
    resolve_canonical(&path)
}

/// Convert a raw path to absolute (join with cwd if relative).
pub fn to_absolute(cwd: &Path, raw: &str) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() { p } else { cwd.join(p) }
}

/// Resolve a path to canonical form, handling non-existent files by
/// walking up the ancestor chain (mirrors sandbox `resolve_canonical`).
fn resolve_canonical(path: &Path) -> Result<PathBuf, ToolIoError> {
    if let Ok(canonical) = path.canonicalize() {
        return Ok(strip_win_prefix(canonical));
    }

    // Walk up to find deepest existing ancestor, then append the rest
    let mut ancestors: Vec<&std::ffi::OsStr> = Vec::new();
    let mut current: &Path = path;
    loop {
        if let Ok(canon) = current.canonicalize() {
            let mut result = strip_win_prefix(canon);
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

    // Fallback: reject obvious `..` traversal
    let path_str = path.to_string_lossy();
    if path_str.contains("..") {
        return Err(ToolIoError::PathDenied(format!(
            "path contains '..': {path_str}"
        )));
    }

    Ok(path.to_path_buf())
}

/// Strip the `\\?\` extended-length prefix that Windows `canonicalize()` adds.
/// This prefix breaks some file operations and is unnecessary for paths < 260 chars.
#[cfg(windows)]
pub fn strip_win_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

#[cfg(not(windows))]
pub fn strip_win_prefix(path: PathBuf) -> PathBuf {
    path
}

fn check_with_policy(
    policy: &ResolvedPolicy,
    path: &Path,
    is_write: bool,
) -> Result<PathBuf, ToolIoError> {
    match loopal_sandbox::path_checker::check_path(policy, path, is_write) {
        PathDecision::Allow => Ok(path.to_path_buf()),
        PathDecision::DenyWrite(reason) => Err(ToolIoError::PermissionDenied(reason)),
        PathDecision::DenyRead(reason) => Err(ToolIoError::PermissionDenied(reason)),
    }
}
