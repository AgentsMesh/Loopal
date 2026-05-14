use loopal_provider_api::{CostTier, QualityTier, SpeedTier, ThinkingCapability};

use super::catalog::ModelEntry;

pub(super) static OPENAI_COMPAT_MODELS: &[ModelEntry] = &[
    ModelEntry {
        id: "deepseek-chat",
        provider: "openai_compat",
        display_name: "DeepSeek V3",
        context_window: 128_000,
        max_output_tokens: 8_192,
        thinking: ThinkingCapability::None,
        speed: SpeedTier::Medium,
        cost: CostTier::Low,
        quality: QualityTier::Standard,
        supports_tools: true,
        supports_vision: false,
        supports_prefill: true,
    },
    ModelEntry {
        id: "deepseek-reasoner",
        provider: "openai_compat",
        display_name: "DeepSeek R1",
        context_window: 128_000,
        max_output_tokens: 8_192,
        thinking: ThinkingCapability::ReasoningEffort,
        speed: SpeedTier::Slow,
        cost: CostTier::Low,
        quality: QualityTier::Standard,
        supports_tools: true,
        supports_vision: false,
        supports_prefill: true,
    },
];
