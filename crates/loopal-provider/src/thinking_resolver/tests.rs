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
        assert!(matches!(
            result,
            Ok(Some(ThinkingConfig::Effort { level: got })) if got == level
        ));
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
