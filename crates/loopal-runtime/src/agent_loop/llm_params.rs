//! Chat parameter construction for LLM requests.
//!
//! Split from llm.rs to keep files under 200 lines.

use loopal_error::Result;
use loopal_message::Message;
use loopal_provider::{get_thinking_capability, resolve_thinking_config};
use loopal_provider_api::ChatParams;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Build chat params from a provided message slice (typically a working copy).
    pub fn prepare_chat_params_with(&self, messages: &[Message]) -> Result<ChatParams> {
        let env_section = super::env_context::build_env_section(
            self.tool_ctx.backend.cwd(),
            self.turn_count,
            self.params.max_turns,
        );
        let full_system_prompt = format!(
            "{}{}{}",
            self.params.system_prompt,
            self.params.mode.system_prompt_suffix(),
            env_section,
        );
        let mut tool_defs = self.params.kernel.tool_definitions();
        if let Some(ref filter) = self.params.tool_filter {
            tool_defs.retain(|t| filter.contains(&t.name));
        }
        let capability = get_thinking_capability(&self.params.model);
        let resolved_thinking = resolve_thinking_config(
            &self.model_config.thinking,
            capability,
            self.model_config.max_output_tokens,
        );
        Ok(ChatParams {
            model: self.params.model.clone(),
            messages: messages.to_vec(),
            system_prompt: full_system_prompt,
            tools: tool_defs,
            max_tokens: self.model_config.max_output_tokens,
            temperature: None,
            thinking: resolved_thinking,
            debug_dump_dir: Some(loopal_config::tmp_dir()),
        })
    }
}
