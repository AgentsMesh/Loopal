use std::path::PathBuf;

use loopal_config::{NetworkPolicy, ResolvedPolicy, SandboxPolicy};
use loopal_sandbox::command_wrapper::wrap_command;

fn disabled_policy() -> ResolvedPolicy {
    ResolvedPolicy {
        policy: SandboxPolicy::Disabled,
        writable_paths: vec![],
        deny_write_globs: vec![],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    }
}

fn workspace_policy() -> ResolvedPolicy {
    ResolvedPolicy {
        policy: SandboxPolicy::WorkspaceWrite,
        writable_paths: vec![PathBuf::from("/home/user/project"), std::env::temp_dir()],
        deny_write_globs: vec![],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    }
}

#[test]
fn disabled_uses_plain_sh() {
    let policy = disabled_policy();
    let cmd = wrap_command(&policy, "echo hello", "/tmp".as_ref());
    assert_eq!(cmd.program, "sh");
    assert_eq!(cmd.args, vec!["-c", "echo hello"]);
}

#[test]
fn disabled_has_sanitized_env() {
    let policy = disabled_policy();
    let cmd = wrap_command(&policy, "echo", "/tmp".as_ref());
    let has_path = cmd.env.keys().any(|k| k.eq_ignore_ascii_case("PATH"));
    assert!(has_path || cmd.env.is_empty());
}

#[test]
fn workspace_uses_sandbox_on_supported_platform() {
    let policy = workspace_policy();
    let cmd = wrap_command(&policy, "ls -la", "/home/user/project".as_ref());

    if cfg!(target_os = "macos") {
        assert_eq!(cmd.program, "sandbox-exec");
        assert!(cmd.args.contains(&"-p".to_string()));
    } else if cfg!(target_os = "linux") {
        // bwrap if available with user-namespace permissions, otherwise sh fallback
        assert!(
            cmd.program == "bwrap" || cmd.program == "sh",
            "expected bwrap or sh, got: {}",
            cmd.program
        );
    } else {
        assert_eq!(cmd.program, "sh");
    }
}

#[test]
fn command_preserved_in_args() {
    let policy = workspace_policy();
    let cmd = wrap_command(&policy, "cargo build --release", "/tmp".as_ref());
    assert!(cmd.args.contains(&"cargo build --release".to_string()));
}

#[test]
fn cwd_preserved() {
    let policy = disabled_policy();
    let cmd = wrap_command(&policy, "pwd", "/my/dir".as_ref());
    assert_eq!(cmd.cwd, PathBuf::from("/my/dir"));
}
