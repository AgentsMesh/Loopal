#[cfg(target_os = "linux")]
mod linux_tests {
    use std::path::PathBuf;

    use loopal_config::{NetworkPolicy, ResolvedPolicy, SandboxPolicy};
    use loopal_sandbox::platform::linux::build_bwrap_args;

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
    fn workspace_has_ro_bind_root() {
        let args = build_bwrap_args(&workspace_policy(), "/home/user/project".as_ref());
        assert!(args.contains(&"--ro-bind".to_string()));
    }

    #[test]
    fn workspace_binds_writable_paths() {
        let args = build_bwrap_args(&workspace_policy(), "/home/user/project".as_ref());
        assert!(args.contains(&"--bind".to_string()));
        assert!(args.contains(&"/home/user/project".to_string()));
        assert!(args.contains(&"/tmp".to_string()));
    }

    #[test]
    fn readonly_no_bind_writable() {
        let args = build_bwrap_args(&readonly_policy(), "/tmp".as_ref());
        // Should have --ro-bind but not --bind
        let bind_count = args.iter().filter(|a| *a == "--bind").count();
        assert_eq!(bind_count, 0);
    }

    #[test]
    fn has_unshare_pid() {
        let args = build_bwrap_args(&workspace_policy(), "/tmp".as_ref());
        assert!(args.contains(&"--unshare-pid".to_string()));
    }

    #[test]
    fn network_allowlist_unshares_net() {
        let mut policy = workspace_policy();
        policy.network.allowed_domains = vec!["github.com".into()];
        let args = build_bwrap_args(&policy, "/tmp".as_ref());
        assert!(args.contains(&"--unshare-net".to_string()));
    }

    #[test]
    fn sets_chdir() {
        let args = build_bwrap_args(&workspace_policy(), "/my/cwd".as_ref());
        let chdir_idx = args.iter().position(|a| a == "--chdir");
        assert!(chdir_idx.is_some());
        let idx = chdir_idx.unwrap();
        assert_eq!(args[idx + 1], "/my/cwd");
    }
}
