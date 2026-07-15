use loopal_provider_api::{EffortLevel, ThinkingConfig};

pub(super) fn reasoning_effort(config: &ThinkingConfig) -> &'static str {
    match config {
        ThinkingConfig::Effort {
            level: EffortLevel::Low,
        } => "low",
        ThinkingConfig::Effort {
            level: EffortLevel::High | EffortLevel::Max,
        } => "high",
        _ => "medium",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_effort_and_degrades_budget() {
        let high = ThinkingConfig::Effort {
            level: EffortLevel::High,
        };
        assert_eq!(reasoning_effort(&high), "high");
        assert_eq!(
            reasoning_effort(&ThinkingConfig::Budget { tokens: 4096 }),
            "medium"
        );
    }
}
