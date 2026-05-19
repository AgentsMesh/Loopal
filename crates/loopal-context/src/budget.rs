use crate::compact_config::COMPACTION_RATIO_PERCENT;
use crate::token_counter::estimate_tokens;

#[derive(Debug, Clone)]
pub struct ContextBudget {
    pub context_window: u32,
    pub system_tokens: u32,
    pub tool_tokens: u32,
    pub output_reserve: u32,
    pub safety_margin: u32,
    /// Token room currently available for messages (window − overhead).
    pub message_budget: u32,
    /// True max_output_tokens from model (for API constraint validation).
    pub max_output_tokens: u32,
}

impl ContextBudget {
    pub fn calculate(
        context_window: u32,
        system_prompt: &str,
        tool_tokens: u32,
        max_output_tokens: u32,
    ) -> Self {
        let system_tokens = estimate_tokens(system_prompt);
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
            max_output_tokens,
        }
    }

    pub fn estimate_tool_tokens(tool_defs: &[loopal_tool_api::ToolDefinition]) -> u32 {
        if tool_defs.is_empty() {
            return 0;
        }
        let per_tool: u32 = tool_defs
            .iter()
            .map(|def| {
                let text = format!("{} {} {}", def.name, def.description, def.input_schema);
                estimate_tokens(&text)
            })
            .sum();
        per_tool + 500
    }

    /// Whether the effective input has crossed the auto-compact threshold.
    /// Caller passes `effective_tokens` — the max of the local estimate and
    /// the most recently reported actual prompt_tokens from the provider.
    pub fn needs_compaction(&self, effective_tokens: u32) -> bool {
        effective_tokens > self.context_window * COMPACTION_RATIO_PERCENT / 100
    }

    /// Clamp max_tokens so that `estimated_input + result <= context_window`.
    pub fn clamp_output_tokens(&self, estimated_input: u32) -> u32 {
        let headroom = self
            .context_window
            .saturating_sub(estimated_input)
            .saturating_sub(self.safety_margin);
        self.max_output_tokens.min(headroom).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_subtracts_all_overhead() {
        let budget = ContextBudget::calculate(200_000, "short system prompt", 500, 16_000);
        assert!(budget.message_budget < 200_000 - 16_000 - 10_000);
        assert!(budget.message_budget > 100_000);
    }

    #[test]
    fn budget_saturates_at_zero() {
        let budget = ContextBudget::calculate(1_000, &"x".repeat(10_000), 5_000, 50_000);
        assert_eq!(budget.message_budget, 0);
    }

    #[test]
    fn needs_compaction_uses_context_window_not_budget() {
        let budget = ContextBudget::calculate(1_000_000, "", 0, 128_000);
        // 80% of 1M = 800K
        assert!(!budget.needs_compaction(799_999));
        assert!(budget.needs_compaction(800_001));
    }

    #[test]
    fn needs_compaction_for_200k_model() {
        let budget = ContextBudget::calculate(200_000, "", 0, 16_000);
        // 80% of 200K = 160K
        assert!(!budget.needs_compaction(159_999));
        assert!(budget.needs_compaction(160_001));
    }

    #[test]
    fn calculate_preserves_max_output_tokens() {
        let budget = ContextBudget::calculate(200_000, "sys", 0, 64_000);
        assert_eq!(budget.max_output_tokens, 64_000);
        assert_eq!(budget.output_reserve, 16_384);
    }

    #[test]
    fn clamp_preserves_when_input_small() {
        let budget = ContextBudget::calculate(200_000, "", 0, 64_000);
        assert_eq!(budget.clamp_output_tokens(50_000), 64_000);
    }

    #[test]
    fn clamp_reduces_when_input_large() {
        let budget = ContextBudget::calculate(200_000, "", 0, 64_000);
        let clamped = budget.clamp_output_tokens(180_000);
        assert!(clamped < 64_000, "should be reduced: {clamped}");
        assert!(clamped <= 200_000 - 180_000 - budget.safety_margin);
    }

    #[test]
    fn clamp_saturates_to_one() {
        let budget = ContextBudget::calculate(200_000, "", 0, 64_000);
        assert_eq!(budget.clamp_output_tokens(300_000), 1);
    }
}
