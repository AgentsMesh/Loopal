//! Semantic security inspection for shell commands.
//!
//! Complements `command_checker` (structural patterns like fork bombs and
//! destructive rm) with higher-level threat detection: remote code execution
//! via piped downloads, credential theft, and system config tampering.

/// Result of a security inspection.
#[derive(Debug, PartialEq, Eq)]
pub enum SecurityVerdict {
    /// No threat detected.
    Allow,
    /// Suspicious but not definitively malicious.
    Warn(String),
    /// Blocked: high-confidence threat pattern.
    Block(String),
}

/// Inspect a shell command for security threats.
pub fn inspect_command(command: &str) -> SecurityVerdict {
    let cmd = command.trim();
    if cmd.is_empty() {
        return SecurityVerdict::Allow;
    }

    // Remote code execution: pipe download to shell
    if has_pipe_to_shell(cmd) {
        return SecurityVerdict::Block(
            "pipe from URL to shell (potential remote code execution)".into(),
        );
    }

    // Eval of remote content
    if has_eval_remote(cmd) {
        return SecurityVerdict::Block(
            "eval of remote content (potential remote code execution)".into(),
        );
    }

    // SSH key injection via base64
    if has_ssh_injection(cmd) {
        return SecurityVerdict::Block(
            "base64 decode to SSH directory (credential injection)".into(),
        );
    }

    // Write to system config directories
    if has_system_config_write(cmd) {
        return SecurityVerdict::Block("write to system configuration directory".into());
    }

    // Suspicious but not blocked
    if cmd.contains("chmod") && cmd.contains("777") {
        return SecurityVerdict::Warn("world-writable permissions (chmod 777)".into());
    }

    SecurityVerdict::Allow
}

/// Detect `curl ... | sh`, `wget ... | bash`, etc.
fn has_pipe_to_shell(cmd: &str) -> bool {
    let shells = ["sh", "bash", "zsh", "dash"];
    let segments: Vec<&str> = cmd.split('|').collect();
    // For each segment after the first, check if it's a shell
    // and if ANY preceding segment contains curl/wget.
    for (i, seg) in segments.iter().enumerate().skip(1) {
        let trimmed = seg.trim();
        let is_shell = shells
            .iter()
            .any(|s| trimmed == *s || trimmed.starts_with(&format!("{s} ")));
        if is_shell {
            let before = segments[..i].join("|");
            if before.contains("curl") || before.contains("wget") {
                return true;
            }
        }
    }
    false
}

/// Detect `eval "$(curl ...)"` or `eval "$(wget ...)"`.
fn has_eval_remote(cmd: &str) -> bool {
    if !cmd.contains("eval") {
        return false;
    }
    cmd.contains("$(curl")
        || cmd.contains("$(wget")
        || cmd.contains("`curl")
        || cmd.contains("`wget")
}

/// Detect `base64 -d >> ~/.ssh/authorized_keys` and similar.
fn has_ssh_injection(cmd: &str) -> bool {
    if !cmd.contains("base64") {
        return false;
    }
    cmd.contains(".ssh") || cmd.contains("authorized_keys")
}

/// Detect writes (>>, >) to /etc/ or sensitive system dirs.
fn has_system_config_write(cmd: &str) -> bool {
    let targets = [">>/etc/", "> /etc/", ">> /etc/", ">/etc/"];
    targets.iter().any(|t| cmd.contains(t))
}
