use loopal_error::{ConfigError, Result};
use loopal_provider_api::{EffortLevel, ThinkingConfig};
use serde_json::{Value, json};

/// Translate a resolved config into the Responses API `reasoning` object.
pub fn to_openai_reasoning(config: &ThinkingConfig) -> Result<Value> {
    let ThinkingConfig::Effort { level } = config else {
        return Err(ConfigError::InvalidValue {
            field: "thinking".into(),
            reason: "OpenAI reasoning requires an effort level".into(),
        }
        .into());
    };
    Ok(json!({
        "effort": effort_name(*level),
        "summary": "auto"
    }))
}

fn effort_name(level: EffortLevel) -> &'static str {
    match level {
        EffortLevel::None => "none",
        EffortLevel::Low => "low",
        EffortLevel::Medium => "medium",
        EffortLevel::High => "high",
        EffortLevel::XHigh => "xhigh",
        EffortLevel::Max => "max",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_efforts_are_preserved() {
        for (level, expected) in [
            (EffortLevel::None, "none"),
            (EffortLevel::Low, "low"),
            (EffortLevel::Medium, "medium"),
            (EffortLevel::High, "high"),
            (EffortLevel::XHigh, "xhigh"),
            (EffortLevel::Max, "max"),
        ] {
            let value = to_openai_reasoning(&ThinkingConfig::Effort { level }).unwrap();
            assert_eq!(value["effort"], expected);
            assert_eq!(value["summary"], "auto");
        }
    }

    #[test]
    fn budget_is_rejected() {
        assert!(to_openai_reasoning(&ThinkingConfig::Budget { tokens: 5_000 }).is_err());
    }
}
