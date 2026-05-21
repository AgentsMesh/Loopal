use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum McpSharing {
    #[default]
    HubSingleton,
    PerAgent,
    SpawnTree,
}

/// Per-server cwd isolation strategy: injects an argument like
/// `--user-data-dir=<cache>/<cache_subdir>/<cwd_hash>` so multiple
/// concurrent agents on different cwds don't fight over the same
/// on-disk state (e.g. Chrome's SingletonLock).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CwdIsolation {
    pub arg: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_subdir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum McpServerConfig {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_mcp_timeout")]
        timeout_ms: u64,
        #[serde(default)]
        sharing: McpSharing,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd_isolation: Option<CwdIsolation>,
    },
    StreamableHttp {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_mcp_timeout")]
        timeout_ms: u64,
        #[serde(default)]
        sharing: McpSharing,
    },
}

impl McpServerConfig {
    pub fn enabled(&self) -> bool {
        match self {
            Self::Stdio { enabled, .. } | Self::StreamableHttp { enabled, .. } => *enabled,
        }
    }

    pub fn timeout_ms(&self) -> u64 {
        match self {
            Self::Stdio { timeout_ms, .. } | Self::StreamableHttp { timeout_ms, .. } => *timeout_ms,
        }
    }

    pub fn sharing(&self) -> McpSharing {
        match self {
            Self::Stdio { sharing, .. } | Self::StreamableHttp { sharing, .. } => *sharing,
        }
    }

    pub fn cwd_isolation(&self) -> Option<&CwdIsolation> {
        match self {
            Self::Stdio { cwd_isolation, .. } => cwd_isolation.as_ref(),
            Self::StreamableHttp { .. } => None,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_mcp_timeout() -> u64 {
    30_000
}
