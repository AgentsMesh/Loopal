use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::mcp::McpServerConfig;
use super::memory::MemoryConfig;
use super::providers::ProvidersConfig;
use crate::fetch_refiner::FetchRefinerConfig;
use crate::harness::HarnessConfig;
use crate::hook::HookConfig;
use crate::sandbox::SandboxConfig;
use crate::telemetry::TelemetryConfig;
use loopal_decision_api::DecisionMode;
use loopal_provider_api::{ModelOverride, TaskType, ThinkingConfig};
use loopal_tool_api::{BgTaskConfig, PermissionMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub model: String,

    #[serde(default)]
    pub model_routing: HashMap<TaskType, String>,

    #[serde(default)]
    pub models: HashMap<String, ModelOverride>,

    pub permission_mode: PermissionMode,

    #[serde(default)]
    pub decision_mode: DecisionMode,

    pub max_context_tokens: u32,

    #[serde(default)]
    pub providers: ProvidersConfig,

    #[serde(default)]
    pub hooks: Vec<HookConfig>,

    #[serde(default)]
    pub mcp_servers: IndexMap<String, McpServerConfig>,

    #[serde(default)]
    pub sandbox: SandboxConfig,

    #[serde(default)]
    pub thinking: ThinkingConfig,

    #[serde(default)]
    pub memory: MemoryConfig,

    #[serde(default)]
    pub harness: HarnessConfig,

    #[serde(default)]
    pub output_style: String,

    #[serde(default)]
    pub telemetry: TelemetryConfig,

    #[serde(default)]
    pub fetch_refiner: FetchRefinerConfig,

    #[serde(default)]
    pub secrets: super::secrets::SecretsSettings,

    #[serde(default)]
    pub goals: GoalSettings,

    #[serde(default)]
    pub compaction: CompactionSettings,

    #[serde(default)]
    pub images: ImageSettings,

    #[serde(default)]
    pub bg_tasks: BgTaskConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "claude-opus-4-7".to_string(),
            model_routing: HashMap::new(),
            models: HashMap::new(),
            permission_mode: PermissionMode::Bypass,
            decision_mode: DecisionMode::Manual,
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
            secrets: super::secrets::SecretsSettings::default(),
            goals: GoalSettings::default(),
            compaction: CompactionSettings::default(),
            images: ImageSettings::default(),
            bg_tasks: BgTaskConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GoalSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_token_budget: Option<u64>,
    pub barren_continuation_limit: u32,
}

impl Default for GoalSettings {
    fn default() -> Self {
        Self {
            default_token_budget: None,
            barren_continuation_limit: 2,
        }
    }
}

/// Tunable knobs for the compaction subsystem. See
/// `crates/loopal-context/src/middleware/microcompact.rs` and `smart_compact.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactionSettings {
    /// Minutes of inactivity after which microcompaction scrubs old
    /// tool_result bodies. Set to 0 to disable. Capped at 1440 (24h) —
    /// anything larger is almost certainly a misconfiguration.
    pub microcompact_idle_minutes: u64,
}

impl CompactionSettings {
    pub const MAX_MICROCOMPACT_IDLE_MINUTES: u64 = 1440;

    /// Clamp out-of-range values. Returns the sanitized copy together with
    /// a list of human-readable warnings so the caller can surface them.
    pub fn sanitize(self) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let mut microcompact_idle_minutes = self.microcompact_idle_minutes;
        if microcompact_idle_minutes > Self::MAX_MICROCOMPACT_IDLE_MINUTES {
            warnings.push(format!(
                "compaction.microcompact_idle_minutes={} exceeds max {}; clamped",
                microcompact_idle_minutes,
                Self::MAX_MICROCOMPACT_IDLE_MINUTES
            ));
            microcompact_idle_minutes = Self::MAX_MICROCOMPACT_IDLE_MINUTES;
        }
        (
            Self {
                microcompact_idle_minutes,
            },
            warnings,
        )
    }
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self {
            microcompact_idle_minutes: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageSettings {
    pub max_bytes: u64,
    pub max_pixels: u64,
    pub inline_threshold_bytes: usize,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            max_bytes: 10 * 1024 * 1024,
            max_pixels: 8192 * 8192,
            inline_threshold_bytes: 256 * 1024,
        }
    }
}
