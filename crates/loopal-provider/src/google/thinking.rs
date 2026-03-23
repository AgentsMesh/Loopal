use loopal_provider_api::{EffortLevel, ThinkingConfig};
use serde_json::{Value, json};

/// Translate a resolved `ThinkingConfig` into Google's thinkingConfig JSON format.
pub fn to_google_thinking(config: &ThinkingConfig) -> Value {
    match config {
        ThinkingConfig::Effort { level } => {
            let budget = match level {
                EffortLevel::Low => 1024,
                EffortLevel::Medium => 8192,
                EffortLevel::High => 16384,
                EffortLevel::Max => 32768,
            };
            json!({"thinkingBudget": budget, "includeThoughts": true})
        }
        ThinkingConfig::Budget { tokens } => {
            json!({"thinkingBudget": tokens, "includeThoughts": true})
        }
        _ => json!({"includeThoughts": true}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_high_maps_to_16384_budget() {
        let result = to_google_thinking(&ThinkingConfig::Effort {
            level: EffortLevel::High,
        });
        assert_eq!(result["thinkingBudget"], 16384);
        assert_eq!(result["includeThoughts"], true);
    }

    #[test]
    fn budget_passes_through() {
        let result = to_google_thinking(&ThinkingConfig::Budget { tokens: 5000 });
        assert_eq!(result["thinkingBudget"], 5000);
    }
}
