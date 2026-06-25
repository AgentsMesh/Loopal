use loopal_context::{degrade_turns_for_wire, estimate_tokens, estimate_turns_tokens};
use loopal_error::Result;
use loopal_provider::{get_thinking_capability, resolve_thinking_config};
use loopal_provider_api::{ChatParams, ContinuationIntent};

use super::runner::AgentLoopRunner;
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
        let capability = get_thinking_capability(&model);
        let resolved_thinking =
            resolve_thinking_config(&self.model_config.thinking, capability, safe_max_tokens);
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
