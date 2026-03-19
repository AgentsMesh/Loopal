use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Top-level sandbox enforcement policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPolicy {
    /// No sandbox enforcement.
    Disabled,
    /// Allow writes only within workspace and temp directories.
    #[default]
    WorkspaceWrite,
    /// Read-only: all writes blocked, only reads allowed.
    ReadOnly,
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
            policy: SandboxPolicy::WorkspaceWrite,
            filesystem: FileSystemPolicy::default(),
            network: NetworkPolicy::default(),
        }
    }
}

/// Filesystem access policy rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FileSystemPolicy {
    /// Additional writable path globs (cwd and tmpdir are always writable).
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
#[derive(Debug, Clone)]
pub struct ResolvedPolicy {
    pub policy: SandboxPolicy,
    pub writable_paths: Vec<PathBuf>,
    pub deny_write_globs: Vec<String>,
    pub deny_read_globs: Vec<String>,
    pub network: NetworkPolicy,
}

/// Decision from path-level sandbox check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathDecision {
    Allow,
    DenyWrite(String),
    DenyRead(String),
}

/// Decision from command-level sandbox check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDecision {
    Allow,
    Deny(String),
}
