use std::path::PathBuf;

use loopal_config::{NetworkPolicy, PathDecision, ResolvedPolicy, SandboxPolicy};
use loopal_sandbox::path_checker::check_path;

fn workspace_policy_with_deny(cwd: &str, deny_write: Vec<String>) -> ResolvedPolicy {
    let cwd_path = PathBuf::from(cwd);
    let cwd_canon = cwd_path.canonicalize().unwrap_or(cwd_path);
    let tmp_canon = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    ResolvedPolicy {
        policy: SandboxPolicy::DefaultWrite,
        writable_paths: vec![cwd_canon, tmp_canon],
        deny_write_globs: deny_write,
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    }
}

#[test]
fn dotdot_traversal_detected() {
    let policy = workspace_policy_with_deny("/home/user/project", vec![]);
    // A path with ".." that resolves outside the writable area
    let path = PathBuf::from("/home/user/project/../../etc/passwd");
    let decision = check_path(&policy, &path, true);
    // Either the canonical resolution catches this (Deny) or it's
    // outside writable dirs (RequiresApproval) — both are non-Allow.
    assert!(
        !matches!(decision, PathDecision::Allow),
        "expected non-Allow, got: {decision:?}"
    );
}

#[test]
fn multiple_deny_globs_checked() {
    let tmp = std::env::temp_dir();
    let policy = workspace_policy_with_deny(
        tmp.to_str().unwrap(),
        vec!["**/*.pem".to_string(), "**/*.key".to_string()],
    );

    let pem_path = tmp.join("cert.pem");
    assert!(matches!(
        check_path(&policy, &pem_path, true),
        PathDecision::RequiresApproval(_)
    ));

    let key_path = tmp.join("server.key");
    assert!(matches!(
        check_path(&policy, &key_path, true),
        PathDecision::RequiresApproval(_)
    ));

    let txt_path = tmp.join("readme.txt");
    assert_eq!(check_path(&policy, &txt_path, true), PathDecision::Allow);
}

#[test]
fn empty_writable_paths_blocks_all_writes() {
    let policy = ResolvedPolicy {
        policy: SandboxPolicy::DefaultWrite,
        writable_paths: vec![],
        deny_write_globs: vec![],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    };
    let path = PathBuf::from("/tmp/some_file.txt");
    assert!(matches!(
        check_path(&policy, &path, true),
        PathDecision::RequiresApproval(_)
    ));
}
