use loopal_context::{degrade_turns_for_wire, estimate_tokens, estimate_turns_tokens};
use loopal_error::Result;
use loopal_provider::{get_thinking_capability, resolve_thinking_config_with_recommendation};
use loopal_provider_api::{ChatParams, ContinuationIntent, ThinkingConfig};

use super::model_config::ModelConfig;
use super::runner::AgentLoopRunner;

fn resolve_thinking_for_request(
    model_config: &ModelConfig,
    model: &str,
    max_output_tokens: u32,
) -> Result<Option<ThinkingConfig>> {
    resolve_thinking_config_with_recommendation(
        &model_config.thinking,
        model_config
            .workflow_preset_thinking_recommendation
            .as_ref(),
        get_thinking_capability(model),
        max_output_tokens,
    )
}
use crate::mode::AgentMode;

impl AgentLoopRunner {
    pub fn prepare_chat_params(
        &self,
        continuation_intent: Option<ContinuationIntent>,
    ) -> Result<ChatParams> {
        let env_section = super::env_context::build_env_section(
            self.tool_ctx.backend.cwd().as_path(),
            self.turn_count,
        );
        let full_system_prompt = format!(
            "{}{}{}",
            self.params.config.system_prompt,
            self.params.config.mode.system_prompt_suffix(),
            env_section,
        );
        let mut tool_defs = self.params.deps.kernel.tool_definitions();

        // reason: tool_filter is now a deny-list (sub-agent / depth-exhausted
        // restrictions). Allow-list semantics would silently strip
        // late-registered MCP tools from sub-agents — see spawn_policy.rs.
        if let Some(ref forbidden) = self.params.config.tool_filter {
            tool_defs.retain(|t| !forbidden.contains(&t.name));
        }
        if self.params.config.mode == AgentMode::Plan
            && let Some(plan_filter) = self.plan_tool_filter()
        {
            tool_defs.retain(|t| plan_filter.contains(&t.name));
        }

        // reason: TurnStore is the wire SSOT but holds the full uncapped
        // record (for persistence). Clone + apply degradation here so the
        // wire payload stays bounded (oversized tool_results capped, old
        // thinking/server blocks stripped). TurnStore on disk is untouched.
        let mut turns = self.turns.store().turns().to_vec();
        degrade_turns_for_wire(&mut turns, self.turns.view().budget());

        // reason: estimate from the SAME degraded turns clone we send on the
        // wire — not from ContextStore's projected messages, which apply a
        // different degradation policy and produce a divergent count. The
        // earlier divergence inflated input estimate and clamped output
        // budget below the true headroom.
        let tool_token_count = loopal_context::ContextBudget::estimate_tool_tokens(&tool_defs);
        let estimated_input =
            estimate_tokens(&full_system_prompt) + tool_token_count + estimate_turns_tokens(&turns);
        let safe_max_tokens = self
            .turns
            .view()
            .budget()
            .clamp_output_tokens(estimated_input);

        let model = self.params.config.model();
        let resolved_thinking =
            resolve_thinking_for_request(&self.model_config, &model, safe_max_tokens)?;
        Ok(ChatParams {
            model,
            turns,
            system_prompt: full_system_prompt,
            tools: tool_defs,
            max_tokens: safe_max_tokens,
            temperature: None,
            thinking: resolved_thinking,
            continuation_intent,
            debug_dump_dir: Some(loopal_config::tmp_dir()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_provider_api::EffortLevel;

    fn model_config(
        configured: ThinkingConfig,
        recommendation: Option<ThinkingConfig>,
    ) -> ModelConfig {
        ModelConfig::from_model_with_recommendation(
            "claude-sonnet-4-6",
            configured,
            recommendation,
            0,
        )
    }

    fn max_recommendation() -> ThinkingConfig {
        ThinkingConfig::Effort {
            level: EffortLevel::Max,
        }
    }

    #[test]
    fn auto_uses_supported_recommendation_without_mutating_setting() {
        let config = model_config(ThinkingConfig::Auto, Some(max_recommendation()));
        let resolved = resolve_thinking_for_request(&config, "claude-sonnet-4-6", 64_000).unwrap();

        assert!(matches!(config.thinking, ThinkingConfig::Auto));
        assert!(matches!(
            resolved,
            Some(ThinkingConfig::Effort {
                level: EffortLevel::Max
            })
        ));
    }

    #[test]
    fn unsupported_recommendation_falls_back_to_active_model_auto() {
        let config = model_config(ThinkingConfig::Auto, Some(max_recommendation()));
        let resolved =
            resolve_thinking_for_request(&config, "claude-sonnet-4-20250514", 10_000).unwrap();

        assert!(matches!(
            resolved,
            Some(ThinkingConfig::Budget { tokens: 8_000 })
        ));
    }

    #[test]
    fn explicit_thinking_wins_and_auto_restores_recommendation() {
        let mut config = model_config(
            ThinkingConfig::Effort {
                level: EffortLevel::Low,
            },
            Some(max_recommendation()),
        );
        let explicit = resolve_thinking_for_request(&config, "claude-sonnet-4-6", 64_000).unwrap();
        assert!(matches!(
            explicit,
            Some(ThinkingConfig::Effort {
                level: EffortLevel::Low
            })
        ));

        config.thinking = ThinkingConfig::Auto;
        let restored = resolve_thinking_for_request(&config, "claude-sonnet-4-6", 64_000).unwrap();
        assert!(matches!(
            restored,
            Some(ThinkingConfig::Effort {
                level: EffortLevel::Max
            })
        ));
    }

    #[test]
    fn model_update_preserves_both_thinking_inputs() {
        let mut config = model_config(ThinkingConfig::Auto, Some(max_recommendation()));
        config.update_model("claude-sonnet-4-20250514");

        assert!(matches!(config.thinking, ThinkingConfig::Auto));
        assert!(matches!(
            config.workflow_preset_thinking_recommendation,
            Some(ThinkingConfig::Effort {
                level: EffortLevel::Max
            })
        ));
    }
}
