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
        ThinkingConfig::Effort { .. } | ThinkingConfig::Budget { .. }
            if capability_supports(config, capability) =>
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

/// Apply a supported recommendation only when the configured value is `Auto`.
pub fn resolve_thinking_config_with_recommendation(
    configured: &ThinkingConfig,
    recommendation: Option<&ThinkingConfig>,
    capability: ThinkingCapability,
    max_output_tokens: u32,
) -> Result<Option<ThinkingConfig>> {
    if !matches!(configured, ThinkingConfig::Auto) {
        return resolve_thinking_config(configured, capability, max_output_tokens);
    }
    let effective = recommendation
        .filter(|candidate| capability_supports(candidate, capability))
        .unwrap_or(configured);
    resolve_thinking_config(effective, capability, max_output_tokens)
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

fn capability_supports(config: &ThinkingConfig, capability: ThinkingCapability) -> bool {
    match config {
        ThinkingConfig::Auto | ThinkingConfig::Disabled => true,
        ThinkingConfig::Effort { level } => supports_effort(capability, *level),
        ThinkingConfig::Budget { .. } => matches!(
            capability,
            ThinkingCapability::BudgetRequired | ThinkingCapability::ThinkingBudget
        ),
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
mod recommendation_tests;
#[cfg(test)]
mod tests;
