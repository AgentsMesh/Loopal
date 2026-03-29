use serde::{Deserialize, Serialize};

use crate::ThinkingCapability;

// ---------------------------------------------------------------------------
// Classification tiers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedTier {
    Fast,
    Medium,
    Slow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostTier {
    Free,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityTier {
    Basic,
    Standard,
    Premium,
}

/// Task types for model routing. Extensible — add variants as needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Main conversation (coding, reasoning, tool-selection, code-review).
    Default,
    /// Context compaction / summarization.
    Summarization,
}

// ---------------------------------------------------------------------------
// Model metadata
// ---------------------------------------------------------------------------

/// Full model metadata used for routing decisions and parameter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub display_name: String,
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub thinking: ThinkingCapability,
    pub speed: SpeedTier,
    pub cost: CostTier,
    pub quality: QualityTier,
    pub supports_tools: bool,
    pub supports_vision: bool,
}

/// User-provided model metadata override (settings.json `models` section).
/// All fields optional — only specified fields override the catalog entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<SpeedTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<QualityTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_vision: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingCapability>,
}
