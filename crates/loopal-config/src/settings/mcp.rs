use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
    },
    StreamableHttp {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_mcp_timeout")]
        timeout_ms: u64,
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
}

fn default_true() -> bool {
    true
}

fn default_mcp_timeout() -> u64 {
    30_000
}
