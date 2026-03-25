use std::path::Path;

use loopal_config::{ResolvedPolicy, SandboxPolicy};

/// System paths that must be writable for basic CLI tool operation.
const SYSTEM_WRITABLE_PATHS: &[&str] = &[
    "/var/tmp", // POSIX /var/tmp (some tools bypass $TMPDIR)
];

/// Build `bwrap` (bubblewrap) arguments for Linux namespace isolation.
pub fn build_bwrap_args(policy: &ResolvedPolicy, cwd: &Path) -> Vec<String> {
    let mut args = Vec::new();

    match policy.policy {
        SandboxPolicy::ReadOnly => {
            // Bind entire filesystem read-only
            args.extend_from_slice(&["--ro-bind".into(), "/".into(), "/".into()]);
            // Still need proc/dev for basic commands
            args.extend_from_slice(&[
                "--proc".into(),
                "/proc".into(),
                "--dev".into(),
                "/dev".into(),
            ]);
            // System paths needed even in read-only mode
            for path in SYSTEM_WRITABLE_PATHS {
                args.extend_from_slice(&["--bind".into(), (*path).into(), (*path).into()]);
            }
        }
        SandboxPolicy::WorkspaceWrite => {
            // Bind root read-only first
            args.extend_from_slice(&["--ro-bind".into(), "/".into(), "/".into()]);
            args.extend_from_slice(&[
                "--proc".into(),
                "/proc".into(),
                "--dev".into(),
                "/dev".into(),
            ]);
            // System paths
            for path in SYSTEM_WRITABLE_PATHS {
                args.extend_from_slice(&["--bind".into(), (*path).into(), (*path).into()]);
            }

            // Bind writable paths
            for path in &policy.writable_paths {
                let p = path.to_string_lossy().into_owned();
                args.extend_from_slice(&["--bind".into(), p.clone(), p]);
            }
        }
        SandboxPolicy::Disabled => {
            // No sandboxing, bind everything read-write
            args.extend_from_slice(&["--bind".into(), "/".into(), "/".into()]);
        }
    }

    // Set working directory
    args.extend_from_slice(&["--chdir".into(), cwd.to_string_lossy().into_owned()]);

    // Unshare namespaces for isolation
    args.push("--unshare-pid".into());

    // Disable network if required
    if !policy.network.allowed_domains.is_empty() || !policy.network.denied_domains.is_empty() {
        // Note: bwrap can only fully disable network, not filter by domain.
        // For domain-level filtering, an additional proxy would be needed.
        // Here we only unshare if there's a strict allowlist.
        if !policy.network.allowed_domains.is_empty() {
            args.push("--unshare-net".into());
        }
    }

    args
}

/// Build the `bwrap` command prefix if bubblewrap is available.
///
/// Returns `None` when `bwrap` is not installed or lacks user-namespace
/// permissions (e.g., unprivileged containers, GitHub Actions runners).
/// The caller falls back to unsandboxed `sh -c` execution.
pub fn build_prefix(policy: &ResolvedPolicy, cwd: &Path) -> Option<(String, Vec<String>)> {
    if !is_bwrap_available() {
        return None;
    }
    let args = build_bwrap_args(policy, cwd);
    Some(("bwrap".to_string(), args))
}

/// Quick probe: spawn `bwrap --ro-bind / / /bin/true` to verify both
/// installation and user-namespace support in a single check.
fn is_bwrap_available() -> bool {
    use std::sync::OnceLock;
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new("bwrap")
            .args(["--ro-bind", "/", "/", "/bin/true"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}
