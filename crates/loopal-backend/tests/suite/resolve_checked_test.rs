//! Tests for LocalBackend: resolve_checked, approve_path, check_sandbox_path.

use std::sync::Arc;

use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_config::{NetworkPolicy, ResolvedPolicy, SandboxPolicy};
use loopal_error::ToolIoError;
use loopal_tool_api::{Backend, ResolvedPath};

fn unique_session_id() -> String {
    format!("test-{}", uuid::Uuid::new_v4().simple())
}

fn make_backend(cwd: &std::path::Path) -> Arc<LocalBackend> {
    let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let policy = ResolvedPolicy {
        policy: SandboxPolicy::DefaultWrite,
        writable_paths: vec![cwd_canon],
        deny_write_globs: vec!["**/.env".to_string()],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    };
    LocalBackend::new(
        cwd.to_path_buf(),
        Some(policy),
        ResourceLimits::default(),
        unique_session_id(),
    )
}

fn make_readonly_backend(cwd: &std::path::Path) -> Arc<LocalBackend> {
    let policy = ResolvedPolicy {
        policy: SandboxPolicy::ReadOnly,
        writable_paths: vec![],
        deny_write_globs: vec![],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    };
    LocalBackend::new(
        cwd.to_path_buf(),
        Some(policy),
        ResourceLimits::default(),
        unique_session_id(),
    )
}

#[cfg(windows)]
fn assert_wire_safe_windows_path(path: &std::path::Path) {
    use std::path::{Component, Prefix};

    let Some(Component::Prefix(prefix)) = path.components().next() else {
        panic!("resolved Windows path must be absolute: {path:?}");
    };
    assert!(
        matches!(prefix.kind(), Prefix::Disk(_) | Prefix::UNC(_, _)),
        "backend-resolved paths must use the ordinary Windows namespace: {path:?}"
    );
}

async fn write_via(
    backend: &Arc<LocalBackend>,
    raw: &str,
    content: &str,
) -> Result<(), ToolIoError> {
    let p = backend.resolve_path(raw, true)?;
    backend.write(&p, content).await?;
    Ok(())
}

// ── check_sandbox_path ───────────────────────────────────────────

/// An absolute path guaranteed to be outside any tempdir on all platforms.
fn outside_cwd_path() -> &'static str {
    if cfg!(windows) {
        r"C:\Windows\System32\evil.exe"
    } else {
        "/usr/local/bin/evil"
    }
}

#[test]
fn check_sandbox_path_returns_none_for_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_backend(dir.path());
    let target = dir.path().join("test.txt");
    assert!(
        backend
            .check_sandbox_path(target.to_str().unwrap(), true)
            .is_none()
    );
}

#[test]
fn check_sandbox_path_returns_reason_for_outside_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_backend(dir.path());
    let reason = backend.check_sandbox_path(outside_cwd_path(), true);
    assert!(reason.is_some());
    assert!(reason.unwrap().contains("outside writable"));
}

#[test]
fn check_sandbox_path_returns_reason_for_deny_glob() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_backend(dir.path());
    let env_path = dir.path().join(".env");
    let reason = backend.check_sandbox_path(env_path.to_str().unwrap(), true);
    assert!(reason.is_some());
}

#[test]
fn check_sandbox_path_returns_none_after_approve() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_backend(dir.path());
    let evil = outside_cwd_path();
    let path = ResolvedPath::from_backend_resolved(std::path::PathBuf::from(evil));

    // Before approval: needs approval
    assert!(backend.check_sandbox_path(evil, true).is_some());

    // After approval: no longer needs approval
    backend.approve_path(&path);
    assert!(backend.check_sandbox_path(evil, true).is_none());
}

// ── resolve_checked (via Backend methods) ────────────────────────

#[cfg(windows)]
#[test]
fn windows_resolver_uses_wire_safe_paths_for_existing_and_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("existing.txt"), "present").unwrap();
    let backend = LocalBackend::new(
        dir.path().to_path_buf(),
        None,
        ResourceLimits::default(),
        unique_session_id(),
    );

    let existing = backend.resolve_path("existing.txt", false).unwrap();
    let missing = backend.resolve_path("missing.txt", true).unwrap();

    assert_wire_safe_windows_path(backend.cwd().as_path());
    assert_wire_safe_windows_path(existing.as_path());
    assert_wire_safe_windows_path(missing.as_path());
}

#[tokio::test]
async fn write_to_allowed_path_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_backend(dir.path());
    let target = dir.path().join("test.txt");
    let result = write_via(&backend, target.to_str().unwrap(), "hello").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn write_outside_cwd_returns_requires_approval() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_backend(dir.path());
    let result = write_via(&backend, outside_cwd_path(), "bad").await;
    assert!(matches!(result, Err(ToolIoError::RequiresApproval(_))));
}

#[tokio::test]
async fn write_to_deny_glob_returns_requires_approval() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_backend(dir.path());
    let env_path = dir.path().join(".env");
    let result = write_via(&backend, env_path.to_str().unwrap(), "SECRET=x").await;
    assert!(matches!(result, Err(ToolIoError::RequiresApproval(_))));
}

#[tokio::test]
async fn write_outside_cwd_succeeds_after_approval() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_backend(dir.path());

    let target_dir = tempfile::tempdir().unwrap();
    let target = target_dir.path().join("approved.txt");

    // Before approval: fails
    assert!(matches!(
        write_via(&backend, target.to_str().unwrap(), "data").await,
        Err(ToolIoError::RequiresApproval(_))
    ));

    // Approve the path
    let target_resolved = ResolvedPath::from_backend_resolved(target.clone());
    backend.approve_path(&target_resolved);

    // After approval: succeeds
    let result = write_via(&backend, target.to_str().unwrap(), "data").await;
    assert!(
        result.is_ok(),
        "expected success after approval, got: {result:?}"
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "data");
}

#[tokio::test]
async fn readonly_mode_hard_denies_writes() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_readonly_backend(dir.path());
    let target = dir.path().join("test.txt");
    let result = write_via(&backend, target.to_str().unwrap(), "data").await;
    assert!(matches!(result, Err(ToolIoError::PermissionDenied(_))));
}

#[tokio::test]
async fn approval_is_session_scoped_across_calls() {
    let dir = tempfile::tempdir().unwrap();
    let backend = make_backend(dir.path());
    let target_dir = tempfile::tempdir().unwrap();
    let target = target_dir.path().join("reuse.txt");

    let target_resolved = ResolvedPath::from_backend_resolved(target.clone());
    backend.approve_path(&target_resolved);

    let r1 = write_via(&backend, target.to_str().unwrap(), "first").await;
    assert!(r1.is_ok());

    let r2 = write_via(&backend, target.to_str().unwrap(), "second").await;
    assert!(r2.is_ok());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "second");
}
