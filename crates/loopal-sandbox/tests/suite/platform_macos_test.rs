#[cfg(target_os = "macos")]
mod macos_tests {
    use std::path::PathBuf;

    use loopal_config::{NetworkPolicy, ResolvedPolicy, SandboxPolicy};
    use loopal_sandbox::platform::macos::generate_seatbelt_profile;

    fn workspace_policy() -> ResolvedPolicy {
        ResolvedPolicy {
            policy: SandboxPolicy::WorkspaceWrite,
            writable_paths: vec![PathBuf::from("/home/user/project"), PathBuf::from("/tmp")],
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
        assert!(profile.contains("(allow file-write* (subpath \"/dev\"))"));
        assert!(profile.contains("(allow file-write* (subpath \"/home/user/project\"))"));
        assert!(profile.contains("(allow file-write* (subpath \"/tmp\"))"));
    }

    #[test]
    fn readonly_profile_has_no_user_writable_paths() {
        let policy = readonly_policy();
        let profile = generate_seatbelt_profile(&policy);
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("(allow file-read*)"));
        // System paths present, but no user-configured writable paths
        assert!(profile.contains("(allow file-write* (subpath \"/dev\"))"));
        assert!(profile.contains("(allow file-write* (subpath \"/private/var/tmp\"))"));
        assert!(!profile.contains("/home/user"));
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

    /// Non-file-system operations are broadly allowed because process-exec
    /// is unrestricted — whitelisting them adds no security but breaks tools.
    #[test]
    fn system_access_rules_are_permissive() {
        let profile = generate_seatbelt_profile(&workspace_policy());

        // Process rules
        assert!(profile.contains("(allow process-exec)"));
        assert!(profile.contains("(allow process-fork)"));
        assert!(profile.contains("(allow signal (target same-sandbox))"));
        assert!(profile.contains("(allow process-info* (target same-sandbox))"));

        // Blanket allows (no whitelists)
        assert!(profile.contains("(allow sysctl-read)"));
        assert!(profile.contains("(allow iokit-open)"));
        assert!(profile.contains("(allow mach-lookup)"));
        assert!(profile.contains("(allow file-map-executable)"));

        // sysctl-write still restricted to JVM's kern.grade_cputype
        assert!(profile.contains("kern.grade_cputype"));
    }

    #[test]
    fn ipc_and_pty_rules_present() {
        let profile = generate_seatbelt_profile(&workspace_policy());
        assert!(profile.contains("(allow ipc-posix-sem)"));
        assert!(profile.contains("(allow ipc-posix-shm*)"));
        assert!(profile.contains("(allow pseudo-tty)"));
        assert!(profile.contains("/dev/ptmx"));
    }
}
