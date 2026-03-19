use loopal_types::sandbox::CommandDecision;

use crate::sensitive_patterns::DANGEROUS_COMMAND_PATTERNS;

/// Check whether a shell command should be allowed or blocked.
///
/// Performs pattern matching against known dangerous commands, detects
/// fork bombs, and flags destructive `rm` invocations.
pub fn check_command(command: &str) -> CommandDecision {
    let trimmed = command.trim();

    if trimmed.is_empty() {
        return CommandDecision::Allow;
    }

    // Check against dangerous command patterns
    for pattern in DANGEROUS_COMMAND_PATTERNS {
        if trimmed.contains(pattern) {
            return CommandDecision::Deny(format!(
                "blocked dangerous pattern: {pattern}"
            ));
        }
    }

    // Check for fork bomb variants
    if is_fork_bomb(trimmed) {
        return CommandDecision::Deny("fork bomb detected".into());
    }

    // Check for destructive rm with force+recursive on system paths
    if is_destructive_rm(trimmed) {
        return CommandDecision::Deny(
            "destructive rm targeting system or home directory".into(),
        );
    }

    // Check for disk/device writes
    if is_device_write(trimmed) {
        return CommandDecision::Deny("direct device write detected".into());
    }

    CommandDecision::Allow
}

/// Detect common fork bomb patterns.
fn is_fork_bomb(cmd: &str) -> bool {
    // Classic bash fork bomb: :(){ :|:& };:
    if cmd.contains(":|:") || cmd.contains("|:&") {
        return true;
    }
    // Perl/Python one-liner fork bombs
    if cmd.contains("fork") && cmd.contains("while") {
        return true;
    }
    false
}

/// Detect destructive `rm` commands targeting root, home, or system dirs.
fn is_destructive_rm(cmd: &str) -> bool {
    // Only check commands starting with rm (possibly preceded by sudo)
    let rm_cmd = cmd.trim_start_matches("sudo ").trim();
    if !rm_cmd.starts_with("rm ") {
        return false;
    }
    let has_rf = rm_cmd.contains("-rf") || rm_cmd.contains("-r -f")
        || rm_cmd.contains("-fr");

    if !has_rf {
        return false;
    }

    // Check for dangerous target paths
    let dangerous_targets = [
        "/", "/*", "/home", "/usr", "/etc", "/var", "/bin",
        "/sbin", "/lib", "/boot", "/sys", "/proc", "/dev",
        "~", "~/",
    ];

    dangerous_targets.iter().any(|target| {
        rm_cmd.ends_with(target) || rm_cmd.contains(&format!("{target} "))
    })
}

/// Detect direct writes to block devices.
fn is_device_write(cmd: &str) -> bool {
    let dev_targets = ["/dev/sda", "/dev/nvme", "/dev/vda", "/dev/hda"];
    dev_targets.iter().any(|dev| cmd.contains(dev))
}
