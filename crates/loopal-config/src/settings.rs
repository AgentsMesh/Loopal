use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::harness::HarnessConfig;
use crate::hook::HookConfig;
use crate::sandbox::SandboxConfig;
use crate::telemetry::TelemetryConfig;
use loopal_provider_api::{ModelOverride, TaskType, ThinkingConfig};
use loopal_tool_api::PermissionMode;

/// Application settings (merged from multiple layers)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Default model identifier
    pub model: String,

    /// Per-task model routing overrides (e.g. summarization → cheap model).
    #[serde(default)]
    pub model_routing: HashMap<TaskType, String>,

    /// Custom model metadata — extends or overrides the built-in catalog.
    #[serde(default)]
    pub models: HashMap<String, ModelOverride>,

    /// Permission mode
    pub permission_mode: PermissionMode,

    /// Maximum context tokens cap (0 = auto: use model's context_window).
    pub max_context_tokens: u32,

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

    /// Harness control parameters — configurable thresholds for the agent control loop.
    #[serde(default)]
    pub harness: HarnessConfig,

    /// Output style override (e.g. "explanatory", "learning"). Empty = default.
    #[serde(default)]
    pub output_style: String,

    /// OpenTelemetry configuration
    #[serde(default)]
    pub telemetry: TelemetryConfig,

    /// Fetch tool LLM-refiner configuration: large pages summarized against the user prompt.
    #[serde(default)]
    pub fetch_refiner: FetchRefinerConfig,

    /// Thread goal feature: per-session long-running objective tracking with
    /// automatic continuation between turns. Disabled by default.
    #[serde(default)]
    pub goals: GoalSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "claude-opus-4-7".to_string(),
            model_routing: HashMap::new(),
            models: HashMap::new(),
            permission_mode: PermissionMode::Bypass,
            max_context_tokens: 0,
            providers: ProvidersConfig::default(),
            hooks: Vec::new(),
            mcp_servers: IndexMap::new(),
            sandbox: SandboxConfig::default(),
            thinking: ThinkingConfig::default(),
            memory: MemoryConfig::default(),
            harness: HarnessConfig::default(),
            output_style: String::new(),
            telemetry: TelemetryConfig::default(),
            fetch_refiner: FetchRefinerConfig::default(),
            goals: GoalSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GoalSettings {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_token_budget: Option<u64>,
    pub barren_continuation_limit: u32,
}

impl Default for GoalSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            default_token_budget: None,
            barren_continuation_limit: 2,
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

/// MCP server configuration, tagged by transport type.
///
/// # Examples (settings.json)
/// ```json
/// {
///   "my-local": { "type": "stdio", "command": "npx", "args": ["-y", "mcp-server"] },
///   "my-remote": { "type": "streamable-http", "url": "https://mcp.example.com/v1" }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum McpServerConfig {
    /// Local subprocess communicating via stdin/stdout.
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
    /// Remote server via Streamable HTTP (with SSE fallback).
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
    /// Whether this server is enabled.
    pub fn enabled(&self) -> bool {
        match self {
            Self::Stdio { enabled, .. } | Self::StreamableHttp { enabled, .. } => *enabled,
        }
    }

    /// Connection timeout in milliseconds.
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

/// Fetch tool LLM-refiner: when a page exceeds `threshold_bytes`,
/// the body is summarized by `model` against the user-supplied `prompt`.
/// Raw markdown is saved to disk so the agent can re-read on demand.
pub use crate::fetch_refiner::FetchRefinerConfig;

/// Auto-memory configuration: controls the Memory tool + Observer sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    /// Enable Memory tool + Observer (default: true)
    pub enabled: bool,
    /// Debounce window in milliseconds — observations arriving within this window
    /// are batched into a single maintainer agent call (default: 2000).
    pub batch_window_ms: u64,
    /// Channel buffer capacity (default: 256).
    pub channel_buffer: usize,
    /// Consolidation interval in days — how often to trigger full memory consolidation
    /// on session start (default: 7). Set to 0 to disable.
    pub consolidation_interval_days: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            // 2s window: allows rapid-fire Memory() calls to batch (50-80% spawn reduction)
            // while keeping latency acceptable for interactive use.
            batch_window_ms: 2000,
            // 256 slots ≈ ~8s of high-frequency observations before backpressure.
            // Dropped observations return "channel full" to the LLM for retry.
            channel_buffer: 256,
            // Weekly full consolidation: balance between freshness and API cost.
            // Set to 0 to disable automatic consolidation.
            consolidation_interval_days: 7,
        }
    }
}
