//! Tests for path.rs: check_requires_approval function.

use std::path::PathBuf;

use loopal_backend::path::check_requires_approval;
use loopal_config::{NetworkPolicy, ResolvedPolicy, SandboxPolicy};

fn workspace_policy(cwd: &str) -> ResolvedPolicy {
    let cwd_path = PathBuf::from(cwd);
    let cwd_canon = cwd_path.canonicalize().unwrap_or(cwd_path);
    let tmp_canon = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    ResolvedPolicy {
        policy: SandboxPolicy::DefaultWrite,
        writable_paths: vec![cwd_canon, tmp_canon],
        deny_write_globs: vec!["**/.env".to_string()],
        deny_read_globs: vec!["**/secret.key".to_string()],
        network: NetworkPolicy::default(),
    }
}

#[test]
fn returns_none_when_policy_is_none() {
    assert!(check_requires_approval(&PathBuf::from("/tmp"), "/etc/hosts", true, None).is_none());
}

#[test]
fn returns_none_for_allowed_path() {
    let tmp = std::env::temp_dir();
    let policy = workspace_policy(tmp.to_str().unwrap());
    let target = tmp.join("allowed.txt");
    assert!(check_requires_approval(&tmp, target.to_str().unwrap(), true, Some(&policy)).is_none());
}

#[test]
fn returns_some_for_write_outside_cwd() {
    let tmp = std::env::temp_dir();
    let policy = workspace_policy(tmp.to_str().unwrap());
    let reason = check_requires_approval(&tmp, "/usr/local/bin/evil", true, Some(&policy));
    assert!(
        reason.is_some(),
        "expected RequiresApproval for outside-cwd write"
    );
    assert!(reason.unwrap().contains("outside writable"));
}

#[test]
fn returns_some_for_deny_write_glob() {
    let tmp = std::env::temp_dir();
    let policy = workspace_policy(tmp.to_str().unwrap());
    let env_path = tmp.join(".env");
    let reason = check_requires_approval(&tmp, env_path.to_str().unwrap(), true, Some(&policy));
    assert!(reason.is_some(), "expected RequiresApproval for .env write");
}

#[test]
fn returns_some_for_deny_read_glob() {
    let tmp = std::env::temp_dir();
    let policy = workspace_policy(tmp.to_str().unwrap());
    let key_path = tmp.join("secret.key");
    let reason = check_requires_approval(&tmp, key_path.to_str().unwrap(), false, Some(&policy));
    assert!(
        reason.is_some(),
        "expected RequiresApproval for deny_read_glob"
    );
}

#[test]
fn returns_none_for_read_not_in_deny_glob() {
    let policy = workspace_policy("/home/user/project");
    let reason = check_requires_approval(
        &PathBuf::from("/home/user/project"),
        "/etc/hosts",
        false,
        Some(&policy),
    );
    assert!(reason.is_none(), "reads outside cwd are allowed by default");
}
