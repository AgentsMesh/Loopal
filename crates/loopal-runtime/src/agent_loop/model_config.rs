//! Aggregates model-related configuration that changes on model switch.
//!
//! Single authority for context-window budget: callers use `build_budget()`
//! instead of constructing `ContextBudget` independently.

use loopal_context::ContextBudget;
use loopal_provider::get_model_info;
use loopal_provider_api::ThinkingConfig;

use super::params::AgentConfig;

/// Model-specific configuration derived from `ModelInfo`.
///
/// Updated on `ControlCommand::ModelSwitch` and `ThinkingSwitch`.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub thinking: ThinkingConfig,
    pub workflow_preset_thinking_recommendation: Option<ThinkingConfig>,
    pub max_context_tokens: u32,
    pub max_output_tokens: u32,
    /// User-configured cap (0 = auto, use model's context_window).
    pub context_tokens_cap: u32,
}

impl ModelConfig {
    pub fn from_agent_config(config: &AgentConfig) -> Self {
        let thinking = config
            .thinking_state
            .as_ref()
            .map(loopal_provider_api::SharedThinkingConfig::get)
            .unwrap_or_else(|| config.thinking_config.clone());
        Self::from_model_with_recommendation(
            &config.model(),
            thinking,
            config.workflow_preset_thinking_recommendation.clone(),
            config.context_tokens_cap,
        )
    }

    /// Build from a model ID, thinking config, and user's context cap.
    pub fn from_model(model: &str, thinking: ThinkingConfig, context_tokens_cap: u32) -> Self {
        Self::from_model_with_recommendation(model, thinking, None, context_tokens_cap)
    }

    pub(super) fn from_model_with_recommendation(
        model: &str,
        thinking: ThinkingConfig,
        workflow_preset_thinking_recommendation: Option<ThinkingConfig>,
        context_tokens_cap: u32,
    ) -> Self {
        let info = get_model_info(model);
        Self {
            thinking,
            workflow_preset_thinking_recommendation,
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

    /// Refresh after a model switch, preserving mutable and preset thinking inputs.
    pub fn update_model(&mut self, model: &str) {
        *self = Self::from_model_with_recommendation(
            model,
            self.thinking.clone(),
            self.workflow_preset_thinking_recommendation.clone(),
            self.context_tokens_cap,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loopal_config::Settings;
    use loopal_kernel::Kernel;
    use loopal_provider_api::{EffortLevel, SharedModelRouter};

    use super::*;
    use crate::SessionManager;
    use crate::agent_loop::{
        AgentConfig, AgentDeps, AgentLoopParamsBuilder, AgentLoopRunner, InterruptHandle,
    };
    use crate::frontend::{
        DecisionContext, DenyAllHandler, UnifiedFrontend, UnsupportedQuestionHandler,
    };

    #[test]
    fn runner_seeds_separate_thinking_inputs_from_agent_config() {
        let base = std::env::temp_dir().join(format!("loopal-model-config-{}", std::process::id()));
        let manager = SessionManager::with_base_dir(base.clone());
        let session = manager
            .create_session_with_id(&base, "claude-sonnet-4-6", "thinking-seed")
            .unwrap();
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(1);
        let (_mailbox_tx, mailbox_rx) = tokio::sync::mpsc::channel(1);
        let (_control_tx, control_rx) = tokio::sync::mpsc::channel(1);
        let frontend = Arc::new(UnifiedFrontend::new(
            None,
            event_tx,
            mailbox_rx,
            control_rx,
            None,
            Box::new(DenyAllHandler),
            Box::new(UnsupportedQuestionHandler),
        ));
        let recommendation = ThinkingConfig::Effort {
            level: EffortLevel::Max,
        };
        let params = AgentLoopParamsBuilder::new(
            AgentConfig {
                router: SharedModelRouter::with_default("claude-sonnet-4-6".into()),
                workflow_preset_thinking_recommendation: Some(recommendation),
                ..Default::default()
            },
            AgentDeps {
                kernel: Arc::new(Kernel::new(Settings::default()).unwrap()),
                frontend,
                session_manager: manager,
                decision_context: DecisionContext::with_cwd(base.to_string_lossy()),
                protected_effect_audit: Arc::new(loopal_tool_api::NoopProtectedEffectAudit),
            },
            session,
            loopal_context::ContextBudget::calculate(200_000, "", 0, 64_000),
            InterruptHandle::new(),
        )
        .build();

        let runner = AgentLoopRunner::new(params);

        assert!(matches!(runner.model_config.thinking, ThinkingConfig::Auto));
        assert!(matches!(
            runner.model_config.workflow_preset_thinking_recommendation,
            Some(ThinkingConfig::Effort {
                level: EffortLevel::Max
            })
        ));
        let _ = std::fs::remove_dir_all(base);
    }
}
