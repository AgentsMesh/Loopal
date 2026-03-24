use crate::token_counter::estimate_tokens;

/// Precise context budget calculation.
///
/// Instead of using a percentage of the raw context window (which ignores system prompt,
/// tool definitions, and output reserve), this calculates the actual token budget
/// available for conversation messages.
#[derive(Debug, Clone)]
pub struct ContextBudget {
    pub context_window: u32,
    pub system_tokens: u32,
    pub tool_tokens: u32,
    pub output_reserve: u32,
    pub safety_margin: u32,
    /// Actual token budget available for messages.
    pub message_budget: u32,
}

impl ContextBudget {
    /// Calculate the message budget by subtracting all non-message overhead.
    ///
    /// - `context_window`: total context window size (e.g. 200_000)
    /// - `system_prompt`: the full system prompt text
    /// - `tool_tokens`: estimated tokens for all tool definitions (use `estimate_tool_tokens`)
    /// - `max_output_tokens`: reserved for model output generation
    pub fn calculate(
        context_window: u32,
        system_prompt: &str,
        tool_tokens: u32,
        max_output_tokens: u32,
    ) -> Self {
        let system_tokens = estimate_tokens(system_prompt);
        // Cap output reserve at 16K — actual output is typically much smaller than
        // max_output_tokens (which can be 64K+ with thinking enabled).
        let output_reserve = max_output_tokens.min(16_384);
        let safety_margin = context_window / 20; // 5%

        let message_budget = context_window
            .saturating_sub(system_tokens)
            .saturating_sub(tool_tokens)
            .saturating_sub(output_reserve)
            .saturating_sub(safety_margin);

        Self {
            context_window,
            system_tokens,
            tool_tokens,
            output_reserve,
            safety_margin,
            message_budget,
        }
    }

    /// Estimate tokens for tool definitions by serializing them.
    /// More accurate than a fixed per-tool heuristic.
    pub fn estimate_tool_tokens(tool_defs: &[loopal_tool_api::ToolDefinition]) -> u32 {
        if tool_defs.is_empty() {
            return 0;
        }
        // Framing overhead (tools array structure) + each tool's JSON definition
        let per_tool: u32 = tool_defs
            .iter()
            .map(|def| {
                let text = format!("{} {} {}", def.name, def.description, def.input_schema);
                estimate_tokens(&text)
            })
            .sum();
        per_tool + 500 // array framing overhead
    }

    /// Whether messages exceed 75% of the budget, triggering LLM summarization.
    pub fn needs_compaction(&self, msg_tokens: u32) -> bool {
        msg_tokens > self.message_budget * 3 / 4
    }

    /// Whether messages exceed 95% of the budget, triggering emergency truncation.
    pub fn needs_emergency(&self, msg_tokens: u32) -> bool {
        msg_tokens > self.message_budget * 19 / 20
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_subtracts_all_overhead() {
        // 200K window, ~500 tool tokens, 16K output
        let budget = ContextBudget::calculate(200_000, "short system prompt", 500, 16_000);
        // system ~4 tokens, tools ~500, output 16000, safety 10000
        assert!(budget.message_budget < 200_000 - 16_000 - 10_000);
        assert!(budget.message_budget > 100_000);
    }

    #[test]
    fn budget_saturates_at_zero() {
        // Tiny window, large overhead → budget should be 0, not underflow
        let budget = ContextBudget::calculate(1_000, &"x".repeat(10_000), 5_000, 50_000);
        assert_eq!(budget.message_budget, 0);
    }

    #[test]
    fn needs_compaction_at_75_percent() {
        let budget = ContextBudget {
            context_window: 200_000,
            system_tokens: 0,
            tool_tokens: 0,
            output_reserve: 0,
            safety_margin: 0,
            message_budget: 100_000,
        };
        assert!(!budget.needs_compaction(74_999));
        assert!(budget.needs_compaction(75_001));
    }

    #[test]
    fn needs_emergency_at_95_percent() {
        let budget = ContextBudget {
            context_window: 200_000,
            system_tokens: 0,
            tool_tokens: 0,
            output_reserve: 0,
            safety_margin: 0,
            message_budget: 100_000,
        };
        assert!(!budget.needs_emergency(94_999));
        assert!(budget.needs_emergency(95_001));
    }
}
