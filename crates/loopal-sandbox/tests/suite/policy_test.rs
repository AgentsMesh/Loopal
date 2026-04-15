use std::path::PathBuf;

use loopal_config::{FileSystemPolicy, NetworkPolicy, SandboxConfig, SandboxPolicy};
use loopal_sandbox::policy::resolve_policy;

#[test]
fn default_policy_is_default_write() {
    let config = SandboxConfig::default();
    assert_eq!(config.policy, SandboxPolicy::DefaultWrite);

    let resolved = resolve_policy(&config, "/tmp".as_ref());
    assert_eq!(resolved.policy, SandboxPolicy::DefaultWrite);
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
fn default_write_includes_cwd() {
    let config = SandboxConfig {
        policy: SandboxPolicy::DefaultWrite,
        filesystem: FileSystemPolicy::default(),
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/home/user/project".as_ref());
    assert!(
        resolved
            .writable_paths
            .contains(&PathBuf::from("/home/user/project"))
    );
}

#[test]
fn default_write_includes_tmpdir() {
    let config = SandboxConfig {
        policy: SandboxPolicy::DefaultWrite,
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
        policy: SandboxPolicy::DefaultWrite,
        filesystem: FileSystemPolicy {
            allow_write: vec!["/extra/path".to_string()],
            deny_write: vec![],
            deny_read: vec![],
        },
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/home/user/project".as_ref());
    assert!(
        resolved
            .writable_paths
            .contains(&PathBuf::from("/extra/path"))
    );
}

#[test]
fn relative_allow_write_resolved_against_cwd() {
    let config = SandboxConfig {
        policy: SandboxPolicy::DefaultWrite,
        filesystem: FileSystemPolicy {
            allow_write: vec!["relative/path".to_string()],
            deny_write: vec![],
            deny_read: vec![],
        },
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/cwd".as_ref());
    assert!(
        resolved
            .writable_paths
            .contains(&PathBuf::from("/cwd/relative/path"))
    );
}

#[test]
fn deny_write_globs_include_defaults_and_user() {
    let config = SandboxConfig {
        policy: SandboxPolicy::DefaultWrite,
        filesystem: FileSystemPolicy {
            allow_write: vec![],
            deny_write: vec!["**/custom_deny".to_string()],
            deny_read: vec![],
        },
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/tmp".as_ref());
    // Should contain default sensitive globs
    assert!(resolved.deny_write_globs.contains(&"**/.env".to_string()));
    // Should also contain user-configured deny
    assert!(
        resolved
            .deny_write_globs
            .contains(&"**/custom_deny".to_string())
    );
}

#[test]
fn default_write_includes_home() {
    let config = SandboxConfig {
        policy: SandboxPolicy::DefaultWrite,
        filesystem: FileSystemPolicy::default(),
        network: NetworkPolicy::default(),
    };

    let resolved = resolve_policy(&config, "/home/user/project".as_ref());
    if let Ok(home) = std::env::var("HOME") {
        let home_canonical = PathBuf::from(&home)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&home));
        assert!(resolved.writable_paths.contains(&home_canonical));
    }
}

#[test]
fn deny_write_globs_include_shell_configs() {
    let config = SandboxConfig::default();
    let resolved = resolve_policy(&config, "/tmp".as_ref());
    assert!(resolved.deny_write_globs.contains(&"**/.bashrc".to_string()));
    assert!(resolved.deny_write_globs.contains(&"**/.zshrc".to_string()));
    assert!(
        resolved
            .deny_write_globs
            .contains(&"**/LaunchAgents/**".to_string())
    );
}

#[test]
fn network_policy_passed_through() {
    let config = SandboxConfig {
        policy: SandboxPolicy::DefaultWrite,
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
