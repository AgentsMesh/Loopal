use std::path::PathBuf;

use loopal_sandbox::policy::resolve_policy;
use loopal_types::sandbox::{
    FileSystemPolicy, NetworkPolicy, SandboxConfig, SandboxPolicy,
};

#[test]
fn default_policy_is_workspace_write() {
    let config = SandboxConfig::default();
    assert_eq!(config.policy, SandboxPolicy::WorkspaceWrite);

    let resolved = resolve_policy(&config, "/tmp".as_ref());
    assert_eq!(resolved.policy, SandboxPolicy::WorkspaceWrite);
    assert!(!resolved.writable_paths.is_empty());
    assert!(!resolved.deny_write_globs.is_empty());
}

#[test]
fn disabled_policy_returns_empty() {
    let config = SandboxConfig {
        policy: SandboxPolicy::Disabled,
        filesystem: FileSystemPolicy::default(),
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/tmp".as_ref());
    assert_eq!(resolved.policy, SandboxPolicy::Disabled);
    assert!(resolved.writable_paths.is_empty());
    assert!(resolved.deny_write_globs.is_empty());
}

#[test]
fn workspace_write_includes_cwd() {
    let config = SandboxConfig {
        policy: SandboxPolicy::WorkspaceWrite,
        filesystem: FileSystemPolicy::default(),
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/home/user/project".as_ref());
    assert!(resolved
        .writable_paths
        .contains(&PathBuf::from("/home/user/project")));
}

#[test]
fn workspace_write_includes_tmpdir() {
    let config = SandboxConfig {
        policy: SandboxPolicy::WorkspaceWrite,
        filesystem: FileSystemPolicy::default(),
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/home/user/project".as_ref());
    let temp_dir = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    assert!(resolved.writable_paths.contains(&temp_dir));
}

#[test]
fn user_allow_write_paths_included() {
    let config = SandboxConfig {
        policy: SandboxPolicy::WorkspaceWrite,
        filesystem: FileSystemPolicy {
            allow_write: vec!["/extra/path".to_string()],
            deny_write: vec![],
            deny_read: vec![],
        },
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/home/user/project".as_ref());
    assert!(resolved
        .writable_paths
        .contains(&PathBuf::from("/extra/path")));
}

#[test]
fn relative_allow_write_resolved_against_cwd() {
    let config = SandboxConfig {
        policy: SandboxPolicy::WorkspaceWrite,
        filesystem: FileSystemPolicy {
            allow_write: vec!["relative/path".to_string()],
            deny_write: vec![],
            deny_read: vec![],
        },
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/cwd".as_ref());
    assert!(resolved
        .writable_paths
        .contains(&PathBuf::from("/cwd/relative/path")));
}

#[test]
fn deny_write_globs_include_defaults_and_user() {
    let config = SandboxConfig {
        policy: SandboxPolicy::WorkspaceWrite,
        filesystem: FileSystemPolicy {
            allow_write: vec![],
            deny_write: vec!["**/custom_deny".to_string()],
            deny_read: vec![],
        },
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/tmp".as_ref());
    // Should contain default sensitive globs
    assert!(resolved
        .deny_write_globs
        .contains(&"**/.env".to_string()));
    // Should also contain user-configured deny
    assert!(resolved
        .deny_write_globs
        .contains(&"**/custom_deny".to_string()));
}

#[test]
fn network_policy_passed_through() {
    let config = SandboxConfig {
        policy: SandboxPolicy::WorkspaceWrite,
        filesystem: FileSystemPolicy::default(),
        network: NetworkPolicy {
            allowed_domains: vec!["github.com".to_string()],
            denied_domains: vec!["evil.com".to_string()],
        },
    };

    let resolved = resolve_policy(&config, "/tmp".as_ref());
    assert_eq!(resolved.network.allowed_domains, vec!["github.com"]);
    assert_eq!(resolved.network.denied_domains, vec!["evil.com"]);
}
