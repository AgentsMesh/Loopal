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

    #[test]
    fn process_rules_aligned_with_codex() {
        let profile = generate_seatbelt_profile(&workspace_policy());
        // signal scoped to same-sandbox (not unrestricted)
        assert!(profile.contains("(allow signal (target same-sandbox))"));
        // process-info scoped to same-sandbox
        assert!(profile.contains("(allow process-info* (target same-sandbox))"));
        // iokit limited to RootDomainUserClient
        assert!(profile.contains("(allow iokit-open"));
        assert!(profile.contains("RootDomainUserClient"));
        // sysctl-write only for JVM
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

    #[test]
    fn mach_lookup_is_whitelist() {
        let profile = generate_seatbelt_profile(&workspace_policy());
        // Must contain specific service names, not blanket mach-lookup
        assert!(profile.contains("com.apple.system.opendirectoryd.libinfo"));
        assert!(profile.contains("com.apple.trustd"));
        assert!(profile.contains("com.apple.SystemConfiguration.DNSConfiguration"));
    }

    #[test]
    fn file_map_executable_for_system_frameworks() {
        let profile = generate_seatbelt_profile(&workspace_policy());
        assert!(profile.contains("(allow file-map-executable"));
        assert!(profile.contains("/System/Library/Frameworks"));
        assert!(profile.contains("/usr/lib"));
    }
}
