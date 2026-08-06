use loopal_error::{ConfigError, Result};
use loopal_provider_api::{EffortLevel, ThinkingConfig};

pub(super) fn reasoning_effort(config: &ThinkingConfig) -> Result<Option<&'static str>> {
    match config {
        ThinkingConfig::Effort { level } => Ok(Some(match level {
            EffortLevel::None => "none",
            EffortLevel::Low => "low",
            EffortLevel::Medium => "medium",
            EffortLevel::High => "high",
            EffortLevel::XHigh => "xhigh",
            EffortLevel::Max => "max",
        })),
        ThinkingConfig::Auto | ThinkingConfig::Disabled => Ok(None),
        ThinkingConfig::Budget { .. } => Err(ConfigError::InvalidValue {
            field: "thinking".into(),
            reason: "reasoning_effort does not accept a token budget".into(),
        }
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_all_effort_values() {
        for (level, expected) in [
            (EffortLevel::None, "none"),
            (EffortLevel::Low, "low"),
            (EffortLevel::Medium, "medium"),
            (EffortLevel::High, "high"),
            (EffortLevel::XHigh, "xhigh"),
            (EffortLevel::Max, "max"),
        ] {
            assert_eq!(
                reasoning_effort(&ThinkingConfig::Effort { level }).unwrap(),
                Some(expected)
            );
        }
    }

    #[test]
    fn budget_is_rejected() {
        assert!(reasoning_effort(&ThinkingConfig::Budget { tokens: 4_096 }).is_err());
    }
}
