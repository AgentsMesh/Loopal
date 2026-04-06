//! Chat parameter construction for LLM requests.
//!
//! Split from llm.rs to keep files under 200 lines.

use loopal_error::Result;
use loopal_message::Message;
use loopal_provider::{get_thinking_capability, resolve_thinking_config};
use loopal_provider_api::ChatParams;

use super::runner::AgentLoopRunner;
use crate::mode::AgentMode;

impl AgentLoopRunner {
    /// Build chat params from a provided message slice (typically a working copy).
    pub fn prepare_chat_params_with(&self, messages: &[Message]) -> Result<ChatParams> {
        let env_section =
            super::env_context::build_env_section(self.tool_ctx.backend.cwd(), self.turn_count);
        let full_system_prompt = format!(
            "{}{}{}",
            self.params.config.system_prompt,
            self.params.config.mode.system_prompt_suffix(),
            env_section,
        );
        let mut tool_defs = self.params.deps.kernel.tool_definitions();

        // Apply user-configured tool filter first.
        if let Some(ref filter) = self.params.config.tool_filter {
            tool_defs.retain(|t| filter.contains(&t.name));
        }
        // In plan mode, further restrict to plan-allowed tools.
        if self.params.config.mode == AgentMode::Plan
            && let Some(plan_filter) = self.plan_tool_filter()
        {
            tool_defs.retain(|t| plan_filter.contains(&t.name));
        }

        let capability = get_thinking_capability(self.params.config.model());
        let resolved_thinking = resolve_thinking_config(
            &self.model_config.thinking,
            capability,
            self.model_config.max_output_tokens,
        );
        Ok(ChatParams {
            model: self.params.config.model().to_string(),
            messages: messages.to_vec(),
            system_prompt: full_system_prompt,
            tools: tool_defs,
            max_tokens: self.model_config.max_output_tokens,
            temperature: None,
            thinking: resolved_thinking,
            debug_dump_dir: Some(loopal_config::tmp_dir()),
        })
    }

    /// Whether the current model requires a user-message suffix for continuation.
    ///
    /// Returns true only when thinking is active AND the provider forbids
    /// assistant-message prefill (currently Anthropic only). OpenAI and Google
    /// reasoning models allow prefill regardless of thinking state, so we
    /// preserve the higher-quality mid-sentence continuation for them.
    pub(super) fn needs_continuation_injection(&self) -> bool {
        let capability = get_thinking_capability(self.params.config.model());
        if !capability.forbids_prefill() {
            return false;
        }
        resolve_thinking_config(
            &self.model_config.thinking,
            capability,
            self.model_config.max_output_tokens,
        )
        .is_some()
    }
}
