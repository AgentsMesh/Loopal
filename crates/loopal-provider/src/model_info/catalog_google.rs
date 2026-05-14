use loopal_provider_api::{CostTier, QualityTier, SpeedTier, ThinkingCapability};

use super::catalog::ModelEntry;

pub(super) static GOOGLE_MODELS: &[ModelEntry] = &[
    ModelEntry {
        id: "gemini-2.0-flash",
        provider: "google",
        display_name: "Gemini 2.0 Flash",
        context_window: 1_000_000,
        max_output_tokens: 8_192,
        thinking: ThinkingCapability::None,
        speed: SpeedTier::Fast,
        cost: CostTier::Low,
        quality: QualityTier::Basic,
        supports_tools: true,
        supports_vision: true,
        supports_prefill: true,
    },
    ModelEntry {
        id: "gemini-2.5-pro-preview-05-06",
        provider: "google",
        display_name: "Gemini 2.5 Pro",
        context_window: 1_000_000,
        max_output_tokens: 65_536,
        thinking: ThinkingCapability::ThinkingBudget,
        speed: SpeedTier::Medium,
        cost: CostTier::Medium,
        quality: QualityTier::Standard,
        supports_tools: true,
        supports_vision: true,
        supports_prefill: true,
    },
    ModelEntry {
        id: "gemini-2.5-flash-preview-04-17",
        provider: "google",
        display_name: "Gemini 2.5 Flash",
        context_window: 1_000_000,
        max_output_tokens: 65_536,
        thinking: ThinkingCapability::ThinkingBudget,
        speed: SpeedTier::Fast,
        cost: CostTier::Low,
        quality: QualityTier::Standard,
        supports_tools: true,
        supports_vision: true,
        supports_prefill: true,
    },
];
