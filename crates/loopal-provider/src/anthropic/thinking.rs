use loopal_provider_api::{EffortLevel, ThinkingConfig};
use serde_json::{Value, json};

/// Translate a resolved `ThinkingConfig` into the Anthropic API `thinking` JSON field.
/// The input must already be resolved (never `Auto`).
///
/// For `Effort` configs, returns `{"type": "adaptive"}`. The effort level is
/// set separately via `to_anthropic_output_config` → `body["output_config"]`.
pub fn to_anthropic_thinking(config: &ThinkingConfig, max_tokens: u32) -> Value {
    match config {
        ThinkingConfig::Effort { .. } => {
            json!({"type": "adaptive"})
        }
        ThinkingConfig::Budget { tokens } => {
            // budget_tokens must be < max_tokens per Anthropic API constraint
            let budget = (*tokens).min(max_tokens.saturating_sub(1));
            json!({"type": "enabled", "budget_tokens": budget})
        }
        ThinkingConfig::Auto | ThinkingConfig::Disabled => {
            json!({"type": "disabled"})
        }
    }
}

/// Extract `output_config` for the Anthropic API request body.
/// Returns `Some({"effort": "high"})` for effort-based configs, `None` otherwise.
pub fn to_anthropic_output_config(config: &ThinkingConfig) -> Option<Value> {
    match config {
        ThinkingConfig::Effort { level } => {
            let effort = match level {
                EffortLevel::Low => "low",
                EffortLevel::Medium => "medium",
                EffortLevel::High => "high",
                EffortLevel::Max => "max",
            };
            Some(json!({"effort": effort}))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_high_maps_to_adaptive() {
        let result = to_anthropic_thinking(
            &ThinkingConfig::Effort {
                level: EffortLevel::High,
            },
            16_384,
        );
        assert_eq!(result["type"], "adaptive");
        // effort is NOT in thinking — it goes in output_config
        assert!(result.get("output_config").is_none());
    }

    #[test]
    fn effort_output_config_returns_effort() {
        let config = ThinkingConfig::Effort {
            level: EffortLevel::Medium,
        };
        let oc = to_anthropic_output_config(&config).unwrap();
        assert_eq!(oc["effort"], "medium");
    }

    #[test]
    fn budget_has_no_output_config() {
        let config = ThinkingConfig::Budget { tokens: 5000 };
        assert!(to_anthropic_output_config(&config).is_none());
    }

    #[test]
    fn budget_clamped_below_max_tokens() {
        let result = to_anthropic_thinking(&ThinkingConfig::Budget { tokens: 20_000 }, 16_384);
        assert_eq!(result["type"], "enabled");
        assert_eq!(result["budget_tokens"], 16_383);
    }
}
