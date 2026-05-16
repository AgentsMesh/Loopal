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
use loopal_tool_api::PermissionMode;

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
    pub goals: GoalSettings,

    #[serde(default)]
    pub secrets: super::secrets::SecretsSettings,
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
            goals: GoalSettings::default(),
            secrets: super::secrets::SecretsSettings::default(),
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
