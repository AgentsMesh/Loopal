use std::path::PathBuf;

use loopal_config::{NetworkPolicy, PathDecision, ResolvedPolicy, SandboxPolicy};
use loopal_sandbox::path_checker::check_path;

fn workspace_policy(cwd: &str) -> ResolvedPolicy {
    let cwd_path = PathBuf::from(cwd);
    let cwd_canon = cwd_path.canonicalize().unwrap_or(cwd_path);
    let tmp_canon = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    ResolvedPolicy {
        policy: SandboxPolicy::WorkspaceWrite,
        writable_paths: vec![cwd_canon, tmp_canon],
        deny_write_globs: vec!["**/.env".to_string()],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    }
}

fn readonly_policy() -> ResolvedPolicy {
    ResolvedPolicy {
        policy: SandboxPolicy::ReadOnly,
        writable_paths: vec![],
        deny_write_globs: vec![],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    }
}

fn disabled_policy() -> ResolvedPolicy {
    ResolvedPolicy {
        policy: SandboxPolicy::Disabled,
        writable_paths: vec![],
        deny_write_globs: vec![],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    }
}

#[test]
fn disabled_allows_anything() {
    let policy = disabled_policy();
    let path = PathBuf::from("/etc/passwd");
    assert_eq!(check_path(&policy, &path, true), PathDecision::Allow);
    assert_eq!(check_path(&policy, &path, false), PathDecision::Allow);
}

#[test]
fn readonly_blocks_all_writes() {
    let policy = readonly_policy();
    let path = PathBuf::from("/tmp/test.txt");
    assert_eq!(check_path(&policy, &path, false), PathDecision::Allow);
    assert!(matches!(
        check_path(&policy, &path, true),
        PathDecision::DenyWrite(_)
    ));
}

#[test]
fn workspace_allows_writes_under_cwd() {
    let tmp = std::env::temp_dir();
    let policy = workspace_policy(tmp.to_str().unwrap());
    let path = tmp.join("subdir/file.txt");
    assert_eq!(check_path(&policy, &path, true), PathDecision::Allow);
}

#[test]
fn workspace_blocks_writes_outside_cwd() {
    let policy = workspace_policy("/home/user/project");
    let path = PathBuf::from("/usr/local/bin/evil");
    assert!(matches!(
        check_path(&policy, &path, true),
        PathDecision::DenyWrite(_)
    ));
}

#[test]
fn deny_write_glob_blocks_env_files() {
    let tmp = std::env::temp_dir();
    let policy = workspace_policy(tmp.to_str().unwrap());
    let path = tmp.join(".env");
    assert!(matches!(
        check_path(&policy, &path, true),
        PathDecision::DenyWrite(_)
    ));
}

#[test]
fn deny_read_glob_blocks_reads() {
    let tmp = std::env::temp_dir();
    let mut policy = workspace_policy(tmp.to_str().unwrap());
    policy.deny_read_globs = vec!["**/secret.txt".to_string()];
    let path = tmp.join("secret.txt");
    assert!(matches!(
        check_path(&policy, &path, false),
        PathDecision::DenyRead(_)
    ));
}

#[test]
fn read_within_workspace_allowed() {
    let policy = workspace_policy("/home/user/project");
    let path = PathBuf::from("/etc/hosts");
    assert_eq!(check_path(&policy, &path, false), PathDecision::Allow);
}
