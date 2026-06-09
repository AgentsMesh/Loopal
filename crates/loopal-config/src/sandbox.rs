use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Top-level sandbox enforcement policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPolicy {
    /// No sandbox enforcement.
    Disabled,
    /// Default write policy: OS sandbox allows all file writes (Bash
    /// commands are gated by the permission system). App-level path_checker
    /// enforces deny_write_globs for File tools, routing sensitive-file
    /// writes through RequiresApproval → user approval.
    #[default]
    #[serde(alias = "workspace_write")]
    DefaultWrite,
    /// Read-only: all writes blocked, only reads allowed.
    ReadOnly,
}

impl std::str::FromStr for SandboxPolicy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "disabled" => Ok(Self::Disabled),
            "default_write" | "workspace_write" => Ok(Self::DefaultWrite),
            "read_only" => Ok(Self::ReadOnly),
            other => Err(format!(
                "invalid sandbox policy '{other}', expected 'disabled', 'default_write', or 'read_only'"
            )),
        }
    }
}

impl std::fmt::Display for SandboxPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Disabled => "disabled",
            Self::DefaultWrite => "default_write",
            Self::ReadOnly => "read_only",
        })
    }
}

/// Sandbox configuration as stored in settings.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SandboxConfig {
    /// Enforcement policy level.
    pub policy: SandboxPolicy,
    /// Filesystem access rules (advanced override).
    pub filesystem: FileSystemPolicy,
    /// Network access rules (advanced override).
    pub network: NetworkPolicy,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            policy: SandboxPolicy::DefaultWrite,
            filesystem: FileSystemPolicy::default(),
            network: NetworkPolicy::default(),
        }
    }
}

/// Filesystem access policy rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FileSystemPolicy {
    /// Additional writable paths for app-level path_checker (cwd, tmpdir, and HOME are included by default).
    pub allow_write: Vec<String>,
    /// Path globs that are always denied for writing.
    pub deny_write: Vec<String>,
    /// Path globs that are denied for reading.
    pub deny_read: Vec<String>,
}

/// Network access policy rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkPolicy {
    /// If non-empty, only these domains are allowed (allowlist mode).
    pub allowed_domains: Vec<String>,
    /// Domains that are always blocked.
    pub denied_domains: Vec<String>,
}

/// Resolved runtime policy computed from config + defaults + cwd.
///
/// `writable_paths` is used by app-level path_checker only (File tools).
/// OS sandbox (seatbelt/bwrap) allows all writes in DefaultWrite mode.
#[derive(Debug, Clone)]
pub struct ResolvedPolicy {
    pub policy: SandboxPolicy,
    /// Baseline writable zones for app-level path_checker (not OS sandbox).
    pub writable_paths: Vec<PathBuf>,
    pub deny_write_globs: Vec<String>,
    pub deny_read_globs: Vec<String>,
    pub network: NetworkPolicy,
}

/// Decision from path-level sandbox check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathDecision {
    Allow,
    /// Hard deny — cannot be overridden (ReadOnly mode, path resolution failure).
    Deny(String),
    /// Soft deny — the operation is outside normal sandbox bounds but can be
    /// approved through the permission system: `Bypass` auto-allows; under
    /// `AskAnyWrite` / `AskDangerous` the handler chain (Manual or Auto) decides.
    RequiresApproval(String),
}

/// Decision from command-level sandbox check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDecision {
    Allow,
    Deny(String),
}

#[cfg(test)]
mod tests {
    use super::SandboxPolicy;

    #[test]
    fn from_str_accepts_known_and_alias() {
        assert_eq!(
            "disabled".parse::<SandboxPolicy>().unwrap(),
            SandboxPolicy::Disabled
        );
        assert_eq!(
            "default_write".parse::<SandboxPolicy>().unwrap(),
            SandboxPolicy::DefaultWrite
        );
        assert_eq!(
            "workspace_write".parse::<SandboxPolicy>().unwrap(),
            SandboxPolicy::DefaultWrite
        );
        assert_eq!(
            "read_only".parse::<SandboxPolicy>().unwrap(),
            SandboxPolicy::ReadOnly
        );
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert!("nope".parse::<SandboxPolicy>().is_err());
    }

    #[test]
    fn display_round_trips_canonical() {
        for p in [
            SandboxPolicy::Disabled,
            SandboxPolicy::DefaultWrite,
            SandboxPolicy::ReadOnly,
        ] {
            assert_eq!(p.to_string().parse::<SandboxPolicy>().unwrap(), p);
        }
    }
}
