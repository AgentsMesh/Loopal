use loopal_context::{estimate_messages_tokens, estimate_tokens};
use loopal_error::Result;
use loopal_message::Message;
use loopal_provider::{get_thinking_capability, resolve_thinking_config};
use loopal_provider_api::{ChatParams, ContinuationIntent};

use super::runner::AgentLoopRunner;
use crate::mode::AgentMode;

impl AgentLoopRunner {
    pub fn prepare_chat_params_with(
        &self,
        messages: &[Message],
        continuation_intent: Option<ContinuationIntent>,
    ) -> Result<ChatParams> {
        let env_section =
            super::env_context::build_env_section(self.tool_ctx.backend.cwd(), self.turn_count);
        let full_system_prompt = format!(
            "{}{}{}",
            self.params.config.system_prompt,
            self.params.config.mode.system_prompt_suffix(),
            env_section,
        );
        let mut tool_defs = self.params.deps.kernel.tool_definitions();

        if let Some(ref filter) = self.params.config.tool_filter {
            tool_defs.retain(|t| filter.contains(&t.name));
        }
        if self.params.config.mode == AgentMode::Plan
            && let Some(plan_filter) = self.plan_tool_filter()
        {
            tool_defs.retain(|t| plan_filter.contains(&t.name));
        }

        // Pre-flight: estimate input tokens and clamp max_tokens to avoid
        // the API's `input + max_tokens > context_window` hard rejection.
        let tool_token_count = loopal_context::ContextBudget::estimate_tool_tokens(&tool_defs);
        let estimated_input = estimate_tokens(&full_system_prompt)
            + tool_token_count
            + estimate_messages_tokens(messages);
        let safe_max_tokens = self
            .params
            .store
            .budget()
            .clamp_output_tokens(estimated_input);

        let capability = get_thinking_capability(self.params.config.model());
        let resolved_thinking =
            resolve_thinking_config(&self.model_config.thinking, capability, safe_max_tokens);
        Ok(ChatParams {
            model: self.params.config.model().to_string(),
            messages: messages.to_vec(),
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
