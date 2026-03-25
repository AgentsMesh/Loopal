//! Platform-specific sandbox wrappers.
//!
//! On macOS, uses `sandbox-exec` with a generated Seatbelt profile.
//! On Linux, uses `bwrap` (bubblewrap) for namespace isolation.

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

use std::path::Path;

use loopal_config::ResolvedPolicy;

/// Build OS-level sandbox command prefix (program + args).
///
/// Returns `(program, args)` to be prepended to the actual command.
/// On unsupported platforms, returns `None`.
pub fn build_sandbox_prefix(policy: &ResolvedPolicy, cwd: &Path) -> Option<(String, Vec<String>)> {
    #[cfg(target_os = "macos")]
    {
        Some(macos::build_prefix(policy, cwd))
    }

    #[cfg(target_os = "linux")]
    {
        linux::build_prefix(policy, cwd)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (policy, cwd);
        None
    }
}
