use std::path::Path;

use loopal_types::sandbox::{ResolvedPolicy, SandboxPolicy};

/// Build `bwrap` (bubblewrap) arguments for Linux namespace isolation.
pub fn build_bwrap_args(policy: &ResolvedPolicy, cwd: &Path) -> Vec<String> {
    let mut args = Vec::new();

    match policy.policy {
        SandboxPolicy::ReadOnly => {
            // Bind entire filesystem read-only
            args.extend_from_slice(&[
                "--ro-bind".into(), "/".into(), "/".into(),
            ]);
            // Still need proc/dev for basic commands
            args.extend_from_slice(&[
                "--proc".into(), "/proc".into(),
                "--dev".into(), "/dev".into(),
            ]);
        }
        SandboxPolicy::WorkspaceWrite => {
            // Bind root read-only first
            args.extend_from_slice(&[
                "--ro-bind".into(), "/".into(), "/".into(),
            ]);
            args.extend_from_slice(&[
                "--proc".into(), "/proc".into(),
                "--dev".into(), "/dev".into(),
            ]);

            // Bind writable paths
            for path in &policy.writable_paths {
                let p = path.to_string_lossy().into_owned();
                args.extend_from_slice(&[
                    "--bind".into(), p.clone(), p,
                ]);
            }
        }
        SandboxPolicy::Disabled => {
            // No sandboxing, bind everything read-write
            args.extend_from_slice(&[
                "--bind".into(), "/".into(), "/".into(),
            ]);
        }
    }

    // Set working directory
    args.extend_from_slice(&[
        "--chdir".into(),
        cwd.to_string_lossy().into_owned(),
    ]);

    // Unshare namespaces for isolation
    args.push("--unshare-pid".into());

    // Disable network if required
    if !policy.network.allowed_domains.is_empty()
        || !policy.network.denied_domains.is_empty()
    {
        // Note: bwrap can only fully disable network, not filter by domain.
        // For domain-level filtering, an additional proxy would be needed.
        // Here we only unshare if there's a strict allowlist.
        if !policy.network.allowed_domains.is_empty() {
            args.push("--unshare-net".into());
        }
    }

    args
}

/// Build the `bwrap` command prefix.
pub fn build_prefix(
    policy: &ResolvedPolicy,
    cwd: &Path,
) -> (String, Vec<String>) {
    let program = "bwrap".to_string();
    let args = build_bwrap_args(policy, cwd);
    (program, args)
}
