//! Aggregates model-related configuration that changes on model switch.
//!
//! Single authority for context-window budget: callers use `build_budget()`
//! instead of constructing `ContextBudget` independently.

use loopal_context::ContextBudget;
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
    /// User-configured cap (0 = auto, use model's context_window).
    pub context_tokens_cap: u32,
}

impl ModelConfig {
    /// Build from a model ID, thinking config, and user's context cap.
    pub fn from_model(model: &str, thinking: ThinkingConfig, context_tokens_cap: u32) -> Self {
        let info = get_model_info(model);
        Self {
            thinking,
            max_context_tokens: info.as_ref().map_or(200_000, |m| m.context_window),
            max_output_tokens: info.as_ref().map_or(16_384, |m| m.max_output_tokens),
            context_tokens_cap,
        }
    }

    /// Effective context window after applying user cap.
    pub fn effective_context_window(&self) -> u32 {
        if self.context_tokens_cap == 0 {
            self.max_context_tokens
        } else {
            self.max_context_tokens.min(self.context_tokens_cap)
        }
    }

    /// Build a `ContextBudget` from this model's capabilities.
    ///
    /// This is the **single entry point** for budget construction — no caller
    /// should use `ContextBudget::calculate()` with a hardcoded window.
    pub fn build_budget(&self, system_prompt: &str, tool_tokens: u32) -> ContextBudget {
        ContextBudget::calculate(
            self.effective_context_window(),
            system_prompt,
            tool_tokens,
            self.max_output_tokens,
        )
    }

    /// Refresh after a model switch, preserving `context_tokens_cap`.
    pub fn update_model(&mut self, model: &str) {
        *self = Self::from_model(model, self.thinking.clone(), self.context_tokens_cap);
    }
}
