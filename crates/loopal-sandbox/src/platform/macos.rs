use std::path::Path;

use loopal_types::sandbox::{ResolvedPolicy, SandboxPolicy};

/// Generate a Seatbelt profile string for `sandbox-exec -p`.
///
/// The profile allows reads everywhere but restricts writes to the
/// configured writable paths.
pub fn generate_seatbelt_profile(policy: &ResolvedPolicy) -> String {
    let mut profile = String::from("(version 1)\n");

    match policy.policy {
        SandboxPolicy::ReadOnly => {
            profile.push_str("(deny default)\n");
            profile.push_str("(allow process-exec)\n");
            profile.push_str("(allow process-fork)\n");
            profile.push_str("(allow sysctl-read)\n");
            profile.push_str("(allow file-read*)\n");
            profile.push_str("(allow mach-lookup)\n");
        }
        SandboxPolicy::WorkspaceWrite => {
            profile.push_str("(deny default)\n");
            profile.push_str("(allow process-exec)\n");
            profile.push_str("(allow process-fork)\n");
            profile.push_str("(allow sysctl-read)\n");
            profile.push_str("(allow file-read*)\n");
            profile.push_str("(allow mach-lookup)\n");

            // Allow writes to configured writable paths
            for path in &policy.writable_paths {
                let path_str = path.to_string_lossy();
                profile.push_str(&format!(
                    "(allow file-write* (subpath \"{path_str}\"))\n"
                ));
            }
        }
        SandboxPolicy::Disabled => {
            profile.push_str("(allow default)\n");
        }
    }

    // Allow network if not restricted
    if policy.network.allowed_domains.is_empty()
        && policy.network.denied_domains.is_empty()
    {
        profile.push_str("(allow network*)\n");
    }

    profile
}

/// Build the `sandbox-exec` command prefix.
pub fn build_prefix(
    policy: &ResolvedPolicy,
    _cwd: &Path,
) -> (String, Vec<String>) {
    let profile = generate_seatbelt_profile(policy);
    let program = "sandbox-exec".to_string();
    let args = vec!["-p".to_string(), profile];
    (program, args)
}
