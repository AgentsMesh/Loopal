use std::path::Path;

use loopal_config::{ResolvedPolicy, SandboxPolicy};

/// System paths that must be writable for basic CLI tool operation.
/// These are platform-invariant safe paths (device files, system temp).
const SYSTEM_WRITABLE_PATHS: &[&str] = &[
    "/dev",             // /dev/null, /dev/tty, /dev/urandom etc.
    "/private/var/tmp", // POSIX /var/tmp (some tools bypass $TMPDIR)
];

/// Append seatbelt rules for essential system writable paths.
fn append_system_write_rules(profile: &mut String) {
    for path in SYSTEM_WRITABLE_PATHS {
        profile.push_str(&format!("(allow file-write* (subpath \"{path}\"))\n"));
    }
}
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
            append_system_write_rules(&mut profile);
        }
        SandboxPolicy::WorkspaceWrite => {
            profile.push_str("(deny default)\n");
            profile.push_str("(allow process-exec)\n");
            profile.push_str("(allow process-fork)\n");
            profile.push_str("(allow sysctl-read)\n");
            profile.push_str("(allow file-read*)\n");
            profile.push_str("(allow mach-lookup)\n");
            append_system_write_rules(&mut profile);

            // Allow writes to configured writable paths
            for path in &policy.writable_paths {
                let path_str = path.to_string_lossy();
                profile.push_str(&format!("(allow file-write* (subpath \"{path_str}\"))\n"));
            }
        }
        SandboxPolicy::Disabled => {
            profile.push_str("(allow default)\n");
        }
    }

    // Allow network if not restricted
    if policy.network.allowed_domains.is_empty() && policy.network.denied_domains.is_empty() {
        profile.push_str("(allow network*)\n");
    }

    profile
}

/// Build the `sandbox-exec` command prefix.
pub fn build_prefix(policy: &ResolvedPolicy, _cwd: &Path) -> (String, Vec<String>) {
    let profile = generate_seatbelt_profile(policy);
    let program = "sandbox-exec".to_string();
    let args = vec!["-p".to_string(), profile];
    (program, args)
}
