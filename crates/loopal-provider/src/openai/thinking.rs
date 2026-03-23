use loopal_provider_api::{EffortLevel, ThinkingConfig};
use serde_json::{Value, json};

/// Translate a resolved `ThinkingConfig` into OpenAI Responses API `reasoning` object.
pub fn to_openai_reasoning(config: &ThinkingConfig) -> Value {
    let effort = match config {
        ThinkingConfig::Effort { level } => match level {
            EffortLevel::Low => "low",
            EffortLevel::Medium => "medium",
            EffortLevel::High | EffortLevel::Max => "high",
        },
        _ => "medium",
    };
    json!({
        "effort": effort,
        "summary": "auto"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_max_maps_to_high() {
        let v = to_openai_reasoning(&ThinkingConfig::Effort {
            level: EffortLevel::Max,
        });
        assert_eq!(v["effort"], "high");
    }

    #[test]
    fn budget_degrades_to_medium() {
        let v = to_openai_reasoning(&ThinkingConfig::Budget { tokens: 5000 });
        assert_eq!(v["effort"], "medium");
    }
}
