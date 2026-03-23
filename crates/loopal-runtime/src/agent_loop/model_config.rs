//! Aggregates model-related configuration that changes on model switch.
//!
//! Extracted from `AgentLoopRunner` to encapsulate thinking config,
//! context window, and output limits behind a single cohesive type.

use loopal_provider::get_model_info;
use loopal_provider_api::ThinkingConfig;

/// Model-specific configuration derived from `ModelInfo`.
///
/// Updated on `ControlCommand::ModelSwitch` and `ThinkingSwitch`.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub thinking: ThinkingConfig,
    pub max_context_tokens: u32,
    pub max_output_tokens: u32,
}

impl ModelConfig {
    /// Build from a model ID and initial thinking config.
    pub fn from_model(model: &str, thinking: ThinkingConfig) -> Self {
        let info = get_model_info(model);
        Self {
            thinking,
            max_context_tokens: info.as_ref().map_or(200_000, |m| m.context_window),
            max_output_tokens: info.as_ref().map_or(16_384, |m| m.max_output_tokens),
        }
    }

    /// Refresh context/output limits after a model switch.
    pub fn update_model(&mut self, model: &str) {
        let info = get_model_info(model);
        self.max_context_tokens = info.as_ref().map_or(200_000, |m| m.context_window);
        self.max_output_tokens = info.as_ref().map_or(16_384, |m| m.max_output_tokens);
    }
}
