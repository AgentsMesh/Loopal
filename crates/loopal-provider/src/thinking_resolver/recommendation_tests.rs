use super::*;

const MAX_RECOMMENDATION: ThinkingConfig = ThinkingConfig::Effort {
    level: EffortLevel::Max,
};
const MAX_OUTPUT_TOKENS: u32 = 10_000;

#[test]
fn max_recommendation_is_used_by_supporting_capabilities() {
    for capability in [
        ThinkingCapability::Adaptive,
        ThinkingCapability::FullReasoningEffort,
        ThinkingCapability::ThinkingBudget,
    ] {
        let resolved = resolve_thinking_config_with_recommendation(
            &ThinkingConfig::Auto,
            Some(&MAX_RECOMMENDATION),
            capability,
            MAX_OUTPUT_TOKENS,
        );
        assert!(matches!(
            resolved,
            Ok(Some(ThinkingConfig::Effort {
                level: EffortLevel::Max
            }))
        ));
    }
}

#[test]
fn unsupported_max_recommendation_falls_back_to_auto() {
    let reasoning = resolve_thinking_config_with_recommendation(
        &ThinkingConfig::Auto,
        Some(&MAX_RECOMMENDATION),
        ThinkingCapability::ReasoningEffort,
        MAX_OUTPUT_TOKENS,
    );
    assert!(matches!(
        reasoning,
        Ok(Some(ThinkingConfig::Effort {
            level: EffortLevel::Medium
        }))
    ));

    let budget = resolve_thinking_config_with_recommendation(
        &ThinkingConfig::Auto,
        Some(&MAX_RECOMMENDATION),
        ThinkingCapability::BudgetRequired,
        MAX_OUTPUT_TOKENS,
    );
    assert!(matches!(
        budget,
        Ok(Some(ThinkingConfig::Budget { tokens: 8_000 }))
    ));

    let none = resolve_thinking_config_with_recommendation(
        &ThinkingConfig::Auto,
        Some(&MAX_RECOMMENDATION),
        ThinkingCapability::None,
        MAX_OUTPUT_TOKENS,
    );
    assert!(matches!(none, Ok(None)));
}

#[test]
fn absent_recommendation_preserves_auto_resolution() {
    for capability in [
        ThinkingCapability::None,
        ThinkingCapability::BudgetRequired,
        ThinkingCapability::Adaptive,
        ThinkingCapability::ReasoningEffort,
        ThinkingCapability::FullReasoningEffort,
        ThinkingCapability::ThinkingBudget,
    ] {
        assert_same_outcome(
            resolve_thinking_config(&ThinkingConfig::Auto, capability, MAX_OUTPUT_TOKENS),
            resolve_thinking_config_with_recommendation(
                &ThinkingConfig::Auto,
                None,
                capability,
                MAX_OUTPUT_TOKENS,
            ),
        );
    }
}

#[test]
fn explicit_configs_preserve_existing_resolution_for_every_capability() {
    let configs = [
        ThinkingConfig::Disabled,
        ThinkingConfig::Effort {
            level: EffortLevel::Low,
        },
        ThinkingConfig::Budget { tokens: 4_096 },
    ];
    let capabilities = [
        ThinkingCapability::None,
        ThinkingCapability::BudgetRequired,
        ThinkingCapability::Adaptive,
        ThinkingCapability::ReasoningEffort,
        ThinkingCapability::FullReasoningEffort,
        ThinkingCapability::ThinkingBudget,
    ];

    for config in &configs {
        for capability in capabilities {
            assert_same_outcome(
                resolve_thinking_config(config, capability, MAX_OUTPUT_TOKENS),
                resolve_thinking_config_with_recommendation(
                    config,
                    Some(&MAX_RECOMMENDATION),
                    capability,
                    MAX_OUTPUT_TOKENS,
                ),
            );
        }
    }
}

fn assert_same_outcome(
    existing: Result<Option<ThinkingConfig>>,
    recommended: Result<Option<ThinkingConfig>>,
) {
    match (existing, recommended) {
        (Ok(existing), Ok(recommended)) => assert_eq!(
            serde_json::to_value(existing).unwrap(),
            serde_json::to_value(recommended).unwrap()
        ),
        (Err(existing), Err(recommended)) => {
            assert_eq!(existing.to_string(), recommended.to_string())
        }
        (existing, recommended) => {
            panic!("resolution changed: existing={existing:?}, recommended={recommended:?}")
        }
    }
}
