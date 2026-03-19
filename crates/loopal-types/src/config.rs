use serde::{Deserialize, Serialize};

use crate::hook::HookConfig;
use crate::permission::PermissionMode;

/// Application settings (merged from 5 layers)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Default model identifier
    pub model: String,

    /// Maximum turns per agent loop
    pub max_turns: u32,

    /// Permission mode
    pub permission_mode: PermissionMode,

    /// Maximum context tokens before compaction
    pub max_context_tokens: u32,

    /// Maximum cost per session (USD)
    pub max_cost: Option<f64>,

    /// Provider configurations
    #[serde(default)]
    pub providers: ProvidersConfig,

    /// Hook configurations
    #[serde(default)]
    pub hooks: Vec<HookConfig>,

    /// MCP server configurations
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            max_turns: 50,
            permission_mode: PermissionMode::Bypass,
            max_context_tokens: 200_000,
            max_cost: None,
            providers: ProvidersConfig::default(),
            hooks: Vec::new(),
            mcp_servers: Vec::new(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name
    pub name: String,
    /// Command to start the server
    pub command: String,
    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}
