use loopal_error::{ConfigError, Result};
use loopal_provider_api::{EffortLevel, ThinkingCapability, ThinkingConfig};

/// Resolve user-facing thinking settings into a provider-valid concrete config.
pub fn resolve_thinking_config(
    config: &ThinkingConfig,
    capability: ThinkingCapability,
    max_output_tokens: u32,
) -> Result<Option<ThinkingConfig>> {
    if capability == ThinkingCapability::None {
        return Ok(None);
    }
    let resolved = match config {
        ThinkingConfig::Disabled => match capability {
            ThinkingCapability::FullReasoningEffort => Some(ThinkingConfig::Effort {
                level: EffortLevel::None,
            }),
            _ => None,
        },
        ThinkingConfig::Auto => auto_config(capability, max_output_tokens),
        ThinkingConfig::Effort { level } if supports_effort(capability, *level) => {
            Some(config.clone())
        }
        ThinkingConfig::Budget { .. }
            if matches!(
                capability,
                ThinkingCapability::BudgetRequired | ThinkingCapability::ThinkingBudget
            ) =>
        {
            Some(config.clone())
        }
        other => {
            return Err(ConfigError::InvalidValue {
                field: "thinking".into(),
                reason: format!("{other:?} is not supported by {capability:?}"),
            }
            .into());
        }
    };
    Ok(resolved)
}

fn auto_config(capability: ThinkingCapability, max_output_tokens: u32) -> Option<ThinkingConfig> {
    match capability {
        ThinkingCapability::Adaptive => Some(ThinkingConfig::Effort {
            level: EffortLevel::High,
        }),
        ThinkingCapability::BudgetRequired => Some(ThinkingConfig::Budget {
            tokens: (max_output_tokens as f64 * 0.8) as u32,
        }),
        ThinkingCapability::ReasoningEffort | ThinkingCapability::FullReasoningEffort => {
            Some(ThinkingConfig::Effort {
                level: EffortLevel::Medium,
            })
        }
        ThinkingCapability::ThinkingBudget => Some(ThinkingConfig::Effort {
            level: EffortLevel::High,
        }),
        ThinkingCapability::None => None,
    }
}

fn supports_effort(capability: ThinkingCapability, level: EffortLevel) -> bool {
    match capability {
        ThinkingCapability::Adaptive => !matches!(level, EffortLevel::None | EffortLevel::XHigh),
        ThinkingCapability::ReasoningEffort => matches!(
            level,
            EffortLevel::Low | EffortLevel::Medium | EffortLevel::High
        ),
        ThinkingCapability::FullReasoningEffort => true,
        ThinkingCapability::ThinkingBudget => {
            !matches!(level, EffortLevel::None | EffortLevel::XHigh)
        }
        ThinkingCapability::None | ThinkingCapability::BudgetRequired => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_reasoning_auto_and_disabled_are_explicit() {
        assert!(matches!(
            resolve_thinking_config(
                &ThinkingConfig::Auto,
                ThinkingCapability::FullReasoningEffort,
                128_000,
            ),
            Ok(Some(ThinkingConfig::Effort {
                level: EffortLevel::Medium
            }))
        ));
        assert!(matches!(
            resolve_thinking_config(
                &ThinkingConfig::Disabled,
                ThinkingCapability::FullReasoningEffort,
                128_000,
            ),
            Ok(Some(ThinkingConfig::Effort {
                level: EffortLevel::None
            }))
        ));
    }

    #[test]
    fn budget_required_auto_uses_80_percent() {
        let result = resolve_thinking_config(
            &ThinkingConfig::Auto,
            ThinkingCapability::BudgetRequired,
            16_384,
        );
        assert!(matches!(
            result,
            Ok(Some(ThinkingConfig::Budget { tokens: 13_107 }))
        ));
    }

    #[test]
    fn full_reasoning_accepts_xhigh_and_max() {
        for level in [EffortLevel::XHigh, EffortLevel::Max] {
            let result = resolve_thinking_config(
                &ThinkingConfig::Effort { level },
                ThinkingCapability::FullReasoningEffort,
                128_000,
            );
            assert!(
                matches!(result, Ok(Some(ThinkingConfig::Effort { level: got })) if got == level)
            );
        }
    }

    #[test]
    fn capability_config_matrix_is_explicit() {
        let cases = [
            (ThinkingCapability::None, ThinkingConfig::Auto, "none"),
            (
                ThinkingCapability::ReasoningEffort,
                ThinkingConfig::Effort {
                    level: EffortLevel::High,
                },
                "effort",
            ),
            (
                ThinkingCapability::Adaptive,
                ThinkingConfig::Effort {
                    level: EffortLevel::Max,
                },
                "effort",
            ),
            (
                ThinkingCapability::ThinkingBudget,
                ThinkingConfig::Budget { tokens: 4_096 },
                "budget",
            ),
            (
                ThinkingCapability::BudgetRequired,
                ThinkingConfig::Budget { tokens: 4_096 },
                "budget",
            ),
        ];
        for (capability, config, expected) in cases {
            let resolved = resolve_thinking_config(&config, capability, 128_000).unwrap();
            assert_eq!(
                match resolved {
                    None => "none",
                    Some(ThinkingConfig::Effort { .. }) => "effort",
                    Some(ThinkingConfig::Budget { .. }) => "budget",
                    Some(_) => "unresolved",
                },
                expected
            );
        }
    }

    #[test]
    fn unsupported_combinations_never_degrade() {
        for (capability, config) in [
            (
                ThinkingCapability::ReasoningEffort,
                ThinkingConfig::Effort {
                    level: EffortLevel::Max,
                },
            ),
            (
                ThinkingCapability::ReasoningEffort,
                ThinkingConfig::Budget { tokens: 5_000 },
            ),
            (
                ThinkingCapability::Adaptive,
                ThinkingConfig::Effort {
                    level: EffortLevel::XHigh,
                },
            ),
            (
                ThinkingCapability::ThinkingBudget,
                ThinkingConfig::Effort {
                    level: EffortLevel::None,
                },
            ),
        ] {
            assert!(resolve_thinking_config(&config, capability, 100_000).is_err());
        }
    }
}
