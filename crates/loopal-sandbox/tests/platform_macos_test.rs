#[cfg(target_os = "macos")]
mod macos_tests {
    use std::path::PathBuf;

    use loopal_sandbox::platform::macos::generate_seatbelt_profile;
    use loopal_types::sandbox::{
        NetworkPolicy, ResolvedPolicy, SandboxPolicy,
    };

    fn workspace_policy() -> ResolvedPolicy {
        ResolvedPolicy {
            policy: SandboxPolicy::WorkspaceWrite,
            writable_paths: vec![
                PathBuf::from("/home/user/project"),
                PathBuf::from("/tmp"),
            ],
            deny_write_globs: vec![],
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

    #[test]
    fn workspace_profile_allows_writes_to_configured_paths() {
        let policy = workspace_policy();
        let profile = generate_seatbelt_profile(&policy);

        assert!(profile.contains("(version 1)"));
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("(allow file-read*)"));
        assert!(profile.contains(
            "(allow file-write* (subpath \"/home/user/project\"))"
        ));
        assert!(profile.contains(
            "(allow file-write* (subpath \"/tmp\"))"
        ));
    }

    #[test]
    fn readonly_profile_no_write_rules() {
        let policy = readonly_policy();
        let profile = generate_seatbelt_profile(&policy);

        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("(allow file-read*)"));
        assert!(!profile.contains("file-write*"));
    }

    #[test]
    fn disabled_allows_all() {
        let policy = ResolvedPolicy {
            policy: SandboxPolicy::Disabled,
            writable_paths: vec![],
            deny_write_globs: vec![],
            deny_read_globs: vec![],
            network: NetworkPolicy::default(),
        };
        let profile = generate_seatbelt_profile(&policy);
        assert!(profile.contains("(allow default)"));
    }

    #[test]
    fn network_allowed_when_no_restrictions() {
        let policy = workspace_policy();
        let profile = generate_seatbelt_profile(&policy);
        assert!(profile.contains("(allow network*)"));
    }
}
