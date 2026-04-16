//! Parse industry-standard `.mcp.json` files into `McpServerConfig` entries.
//!
//! Format: `{ "mcpServers": { "<name>": { "command": "...", ... } } }`
//! - Servers without a `type` field default to `stdio`.
//! - Loopal extensions (`enabled`, `timeout_ms`) are optional.

use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;
use serde::Deserialize;

use crate::settings::McpServerConfig;

#[derive(Deserialize)]
struct McpJsonFile {
    #[serde(default, rename = "mcpServers")]
    servers: IndexMap<String, McpJsonEntry>,
}

#[derive(Deserialize)]
struct McpJsonEntry {
    #[serde(rename = "type")]
    server_type: Option<String>,
    // stdio
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    // streamable-http
    url: Option<String>,
    #[serde(default)]
    headers: HashMap<String, String>,
    // Loopal extensions
    enabled: Option<bool>,
    timeout_ms: Option<u64>,
}

const DEFAULT_MCP_TIMEOUT: u64 = 30_000;

/// Load `.mcp.json` from `path`, returning parsed servers.
///
/// Missing file or parse errors are handled gracefully (logged, not fatal).
pub fn load_mcp_json(path: &Path) -> IndexMap<String, McpServerConfig> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return IndexMap::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), "failed to read .mcp.json: {e}");
            return IndexMap::new();
        }
    };

    let file: McpJsonFile = match serde_json::from_str(&contents) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(path = %path.display(), "invalid .mcp.json: {e}");
            return IndexMap::new();
        }
    };

    let mut result = IndexMap::new();
    for (name, entry) in file.servers {
        if let Some(config) = convert_entry(&name, entry) {
            result.insert(name, config);
        }
    }
    result
}

fn convert_entry(name: &str, entry: McpJsonEntry) -> Option<McpServerConfig> {
    let enabled = entry.enabled.unwrap_or(true);
    let timeout_ms = entry.timeout_ms.unwrap_or(DEFAULT_MCP_TIMEOUT);
    let server_type = entry.server_type.as_deref().unwrap_or("stdio");

    match server_type {
        "stdio" => {
            let Some(command) = entry.command else {
                tracing::warn!(server = %name, "stdio server missing 'command', skipping");
                return None;
            };
            Some(McpServerConfig::Stdio {
                command,
                args: entry.args,
                env: entry.env,
                enabled,
                timeout_ms,
            })
        }
        "streamable-http" => {
            let Some(url) = entry.url else {
                tracing::warn!(server = %name, "streamable-http server missing 'url', skipping");
                return None;
            };
            Some(McpServerConfig::StreamableHttp {
                url,
                headers: entry.headers,
                enabled,
                timeout_ms,
            })
        }
        other => {
            tracing::warn!(server = %name, r#type = %other, "unknown server type, skipping");
            None
        }
    }
}
