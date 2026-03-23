use loopal_provider_api::{EffortLevel, ThinkingCapability, ThinkingConfig};

/// Resolve a user-facing `ThinkingConfig` into a concrete config for providers.
///
/// `Auto` is expanded into provider-appropriate defaults; `None` capability
/// always returns `None` (no thinking). Providers receive only concrete
/// configs — never `Auto`.
pub fn resolve_thinking_config(
    config: &ThinkingConfig,
    capability: ThinkingCapability,
    max_output_tokens: u32,
) -> Option<ThinkingConfig> {
    if capability == ThinkingCapability::None {
        return None;
    }
    match config {
        ThinkingConfig::Disabled => None,
        ThinkingConfig::Auto => match capability {
            ThinkingCapability::Adaptive => Some(ThinkingConfig::Effort {
                level: EffortLevel::High,
            }),
            ThinkingCapability::BudgetRequired => {
                let budget = (max_output_tokens as f64 * 0.8) as u32;
                Some(ThinkingConfig::Budget { tokens: budget })
            }
            ThinkingCapability::ReasoningEffort => Some(ThinkingConfig::Effort {
                level: EffortLevel::Medium,
            }),
            ThinkingCapability::ThinkingBudget => Some(ThinkingConfig::Effort {
                level: EffortLevel::High,
            }),
            ThinkingCapability::None => None,
        },
        other => Some(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_with_none_capability_returns_none() {
        let result =
            resolve_thinking_config(&ThinkingConfig::Auto, ThinkingCapability::None, 16_384);
        assert!(result.is_none());
    }

    #[test]
    fn auto_with_adaptive_returns_high_effort() {
        let result =
            resolve_thinking_config(&ThinkingConfig::Auto, ThinkingCapability::Adaptive, 16_384);
        match result {
            Some(ThinkingConfig::Effort { level }) => {
                assert_eq!(level, EffortLevel::High);
            }
            other => panic!("expected Effort(High), got {other:?}"),
        }
    }

    #[test]
    fn auto_with_budget_required_returns_80pct_budget() {
        let result = resolve_thinking_config(
            &ThinkingConfig::Auto,
            ThinkingCapability::BudgetRequired,
            16_384,
        );
        match result {
            Some(ThinkingConfig::Budget { tokens }) => {
                assert_eq!(tokens, 13107); // 16384 * 0.8
            }
            other => panic!("expected Budget, got {other:?}"),
        }
    }

    #[test]
    fn disabled_always_returns_none() {
        let result = resolve_thinking_config(
            &ThinkingConfig::Disabled,
            ThinkingCapability::Adaptive,
            16_384,
        );
        assert!(result.is_none());
    }

    #[test]
    fn explicit_effort_passes_through() {
        let result = resolve_thinking_config(
            &ThinkingConfig::Effort {
                level: EffortLevel::Max,
            },
            ThinkingCapability::Adaptive,
            16_384,
        );
        match result {
            Some(ThinkingConfig::Effort { level }) => {
                assert_eq!(level, EffortLevel::Max);
            }
            other => panic!("expected Effort(Max), got {other:?}"),
        }
    }

    #[test]
    fn explicit_budget_passes_through() {
        let result = resolve_thinking_config(
            &ThinkingConfig::Budget { tokens: 5000 },
            ThinkingCapability::ReasoningEffort,
            100_000,
        );
        match result {
            Some(ThinkingConfig::Budget { tokens }) => {
                assert_eq!(tokens, 5000);
            }
            other => panic!("expected Budget(5000), got {other:?}"),
        }
    }
}
