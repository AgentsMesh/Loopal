use std::path::Path;
use std::sync::OnceLock;

use loopal_config::{ResolvedPolicy, SandboxPolicy};

/// Static base policy loaded from the `.sbpl` file at compile time.
/// Contains: deny-default, process/sysctl/iokit/mach/ipc/pty rules,
/// system writable paths, and framework executable-mapping rules.
const BASE_POLICY: &str = include_str!("seatbelt_base.sbpl");

/// Cached result of the `sandbox-exec` availability probe.
static SANDBOX_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Test whether `sandbox-exec` is functional on this system.
///
/// Recent macOS versions disable the Seatbelt API for non-Apple processes,
/// causing `sandbox-exec` to fail with exit code 71 ("Operation not permitted").
/// This probe runs once and caches the result for the lifetime of the process.
fn is_sandbox_exec_available() -> bool {
    *SANDBOX_AVAILABLE.get_or_init(|| {
        std::process::Command::new("sandbox-exec")
            .args(["-p", "(version 1)(allow default)", "true"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

/// Generate a macOS Seatbelt profile string from the resolved policy.
///
/// Composes the static base policy with dynamic sections for file-read,
/// file-write (writable paths), and network access.
pub fn generate_seatbelt_profile(policy: &ResolvedPolicy) -> String {
    if policy.policy == SandboxPolicy::Disabled {
        return "(version 1)\n(allow default)\n".to_string();
    }

    let mut profile = BASE_POLICY.to_string();

    // file-read*: full read access (Codex supports per-root restrictions,
    // but we keep it simple for now).
    profile.push_str("\n; --- Dynamic: file access ---\n");
    profile.push_str("(allow file-read*)\n");

    // file-write*: per-path restrictions for WorkspaceWrite
    if policy.policy == SandboxPolicy::WorkspaceWrite {
        for path in &policy.writable_paths {
            let path_str = path.to_string_lossy();
            profile.push_str(&format!("(allow file-write* (subpath \"{path_str}\"))\n"));
        }
    }

    // network
    if policy.network.allowed_domains.is_empty() && policy.network.denied_domains.is_empty() {
        profile.push_str("\n; --- Dynamic: network ---\n");
        profile.push_str("(allow network*)\n");
    }

    profile
}

/// Build the `sandbox-exec` command prefix.
///
/// Returns `None` when `sandbox-exec` is unavailable on this system
/// (e.g. recent macOS versions that block the Seatbelt API).
pub fn build_prefix(policy: &ResolvedPolicy, _cwd: &Path) -> Option<(String, Vec<String>)> {
    if !is_sandbox_exec_available() {
        return None;
    }
    let profile = generate_seatbelt_profile(policy);
    let program = "sandbox-exec".to_string();
    let args = vec!["-p".to_string(), profile];
    Some((program, args))
}
