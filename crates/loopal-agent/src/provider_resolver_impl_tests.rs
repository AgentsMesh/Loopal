use loopal_provider_api::{EffortLevel, SharedThinkingConfig, ThinkingConfig};
use loopal_tool_api::OneShotChatEffort;

use super::one_shot_thinking;

#[test]
fn default_one_shot_keeps_thinking_disabled() {
    assert!(
        one_shot_thinking(
            "claude-sonnet-4-6",
            4_096,
            OneShotChatEffort::Default,
            &ThinkingConfig::Auto,
        )
        .is_none()
    );
}

#[test]
fn ultracode_effort_is_resolved_per_model_capability() {
    assert!(matches!(
        one_shot_thinking(
            "claude-sonnet-4-6",
            4_096,
            OneShotChatEffort::Max,
            &ThinkingConfig::Auto,
        ),
        Some(ThinkingConfig::Effort {
            level: EffortLevel::Max
        })
    ));
    assert!(matches!(
        one_shot_thinking(
            "claude-sonnet-4-20250514",
            10_000,
            OneShotChatEffort::Max,
            &ThinkingConfig::Auto,
        ),
        Some(ThinkingConfig::Budget { tokens: 8_000 })
    ));
    assert!(
        one_shot_thinking(
            "unknown-model",
            4_096,
            OneShotChatEffort::Max,
            &ThinkingConfig::Auto,
        )
        .is_none()
    );
}

#[test]
fn live_explicit_config_overrides_ultracode_recommendation() {
    let state = SharedThinkingConfig::new(ThinkingConfig::Auto);
    let reader = state.reader();
    assert!(matches!(
        one_shot_thinking(
            "claude-sonnet-4-6",
            4_096,
            OneShotChatEffort::Max,
            &reader.get(),
        ),
        Some(ThinkingConfig::Effort {
            level: EffortLevel::Max
        })
    ));

    state.set(ThinkingConfig::Disabled);
    assert!(
        one_shot_thinking(
            "claude-sonnet-4-6",
            4_096,
            OneShotChatEffort::Max,
            &reader.get(),
        )
        .is_none()
    );

    state.set(ThinkingConfig::Effort {
        level: EffortLevel::Low,
    });
    assert!(matches!(
        one_shot_thinking(
            "claude-sonnet-4-6",
            4_096,
            OneShotChatEffort::Max,
            &reader.get(),
        ),
        Some(ThinkingConfig::Effort {
            level: EffortLevel::Low
        })
    ));
}
