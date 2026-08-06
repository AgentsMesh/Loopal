use serde::{Deserialize, Serialize};

/// What kind of thinking/reasoning the model natively supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThinkingCapability {
    /// No thinking support (e.g. GPT-4o, Haiku).
    None,
    /// Requires explicit budget_tokens (Anthropic Sonnet 4, Opus 4).
    BudgetRequired,
    /// Adaptive thinking with effort levels (Anthropic Sonnet 4.6, Opus 4.6).
    Adaptive,
    /// OpenAI reasoning_effort parameter (o1/o3/o3-mini/o4-mini).
    ReasoningEffort,
    /// OpenAI full reasoning effort range (GPT-5.6 family).
    FullReasoningEffort,
    /// Google thinkingBudget parameter (Gemini 2.5 Pro/Flash).
    ThinkingBudget,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkingConfig {
    /// Auto-detect based on model capability (default).
    #[default]
    Auto,
    /// Thinking disabled.
    Disabled,
    /// Unified effort level across providers.
    Effort { level: EffortLevel },
    /// Explicit budget in tokens.
    Budget { tokens: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffortLevel {
    None,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_levels_round_trip_as_lowercase_json() {
        for (level, expected) in [
            (EffortLevel::None, "none"),
            (EffortLevel::Low, "low"),
            (EffortLevel::Medium, "medium"),
            (EffortLevel::High, "high"),
            (EffortLevel::XHigh, "xhigh"),
            (EffortLevel::Max, "max"),
        ] {
            let json = serde_json::to_string(&level).unwrap();
            assert_eq!(json, format!("\"{expected}\""));
            assert_eq!(serde_json::from_str::<EffortLevel>(&json).unwrap(), level);
        }
    }
}
