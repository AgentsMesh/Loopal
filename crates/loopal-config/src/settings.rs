use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::hook::HookConfig;
use crate::sandbox::SandboxConfig;
use loopal_provider_api::ThinkingConfig;
use loopal_tool_api::PermissionMode;

/// Application settings (merged from multiple layers)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Default model identifier
    pub model: String,

    /// Model for auxiliary tasks (compaction, summarization).
    /// Defaults to the main model when not set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_model: Option<String>,

    /// Maximum turns per agent loop
    pub max_turns: u32,

    /// Permission mode
    pub permission_mode: PermissionMode,

    /// Maximum context tokens cap (0 = auto: use model's context_window).
    pub max_context_tokens: u32,

    /// Maximum cost per session (USD)
    pub max_cost: Option<f64>,

    /// Provider configurations
    #[serde(default)]
    pub providers: ProvidersConfig,

    /// Hook configurations
    #[serde(default)]
    pub hooks: Vec<HookConfig>,

    /// MCP server configurations (name → config)
    #[serde(default)]
    pub mcp_servers: IndexMap<String, McpServerConfig>,

    /// Sandbox configuration
    #[serde(default)]
    pub sandbox: SandboxConfig,

    /// Thinking/reasoning configuration (default: Auto)
    #[serde(default)]
    pub thinking: ThinkingConfig,

    /// Auto-memory configuration
    #[serde(default)]
    pub memory: MemoryConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            compact_model: None,
            max_turns: 50,
            permission_mode: PermissionMode::Bypass,
            max_context_tokens: 0,
            max_cost: None,
            providers: ProvidersConfig::default(),
            hooks: Vec::new(),
            mcp_servers: IndexMap::new(),
            sandbox: SandboxConfig::default(),
            thinking: ThinkingConfig::default(),
            memory: MemoryConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub anthropic: Option<ProviderConfig>,
    pub openai: Option<ProviderConfig>,
    pub google: Option<ProviderConfig>,
    pub openai_compat: Vec<OpenAiCompatConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API key (can also use env var)
    pub api_key: Option<String>,
    /// API key environment variable name
    pub api_key_env: Option<String>,
    /// Custom base URL
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiCompatConfig {
    /// Provider name identifier
    pub name: String,
    /// Base URL
    pub base_url: String,
    /// API key
    pub api_key: Option<String>,
    /// API key environment variable name
    pub api_key_env: Option<String>,
    /// Model prefix (e.g., "ollama/")
    pub model_prefix: Option<String>,
}

/// MCP server configuration (name is the key in the outer map)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Command to start the server
    pub command: String,
    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Whether this server is enabled (use false to disable an inherited server)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Connection timeout in milliseconds
    #[serde(default = "default_mcp_timeout")]
    pub timeout_ms: u64,
}

fn default_true() -> bool {
    true
}

fn default_mcp_timeout() -> u64 {
    30_000
}

/// Auto-memory configuration: controls the Memory tool + Observer sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    /// Enable Memory tool + Observer (default: true)
    pub enabled: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}
