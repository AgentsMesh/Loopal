use loopal_provider_api::{CostTier, ModelInfo, QualityTier, SpeedTier, ThinkingCapability};

use super::catalog_anthropic::ANTHROPIC_MODELS;
use super::catalog_google::GOOGLE_MODELS;
use super::catalog_openai::OPENAI_MODELS;
use super::catalog_openai_compat::OPENAI_COMPAT_MODELS;

pub(crate) struct ModelEntry {
    pub id: &'static str,
    pub provider: &'static str,
    pub display_name: &'static str,
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub thinking: ThinkingCapability,
    pub speed: SpeedTier,
    pub cost: CostTier,
    pub quality: QualityTier,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub supports_prefill: bool,
}

impl ModelEntry {
    pub fn to_model_info(&self) -> ModelInfo {
        ModelInfo {
            id: self.id.to_string(),
            provider: self.provider.to_string(),
            display_name: self.display_name.to_string(),
            context_window: self.context_window,
            max_output_tokens: self.max_output_tokens,
            thinking: self.thinking,
            speed: self.speed,
            cost: self.cost,
            quality: self.quality,
            supports_tools: self.supports_tools,
            supports_vision: self.supports_vision,
            supports_prefill: self.supports_prefill,
        }
    }
}

pub(crate) fn known_models() -> impl Iterator<Item = &'static ModelEntry> {
    ANTHROPIC_MODELS
        .iter()
        .chain(OPENAI_MODELS.iter())
        .chain(GOOGLE_MODELS.iter())
        .chain(OPENAI_COMPAT_MODELS.iter())
}
