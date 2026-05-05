use loopal_provider::model_info::overlay;
use loopal_provider_api::{CostTier, ModelOverride, QualityTier, SpeedTier, ThinkingCapability};

#[test]
fn test_synthesize_defaults() {
    let info = overlay::synthesize("my-model", "my-provider");
    assert_eq!(info.id, "my-model");
    assert_eq!(info.provider, "my-provider");
    assert_eq!(info.display_name, "my-model");
    assert_eq!(info.context_window, 128_000);
    assert_eq!(info.max_output_tokens, 16_384);
    assert_eq!(info.thinking, ThinkingCapability::None);
    assert_eq!(info.speed, SpeedTier::Medium);
    assert_eq!(info.cost, CostTier::Medium);
    assert_eq!(info.quality, QualityTier::Standard);
    assert!(info.supports_tools);
    assert!(!info.supports_vision);
}

#[test]
fn test_get_user_model_returns_none_for_unknown() {
    assert!(overlay::get_user_model("this-model-definitely-does-not-exist-xyz").is_none());
}

// -- apply_override --------------------------------------------------------

#[test]
fn test_apply_override_empty_changes_nothing() {
    let base = overlay::synthesize("m", "p");
    let empty: ModelOverride = serde_json::from_str("{}").unwrap();
    let result = overlay::apply_override(base.clone(), &empty);

    assert_eq!(result.provider, base.provider);
    assert_eq!(result.context_window, base.context_window);
    assert_eq!(result.speed, base.speed);
    assert_eq!(result.supports_tools, base.supports_tools);
}

#[test]
fn test_apply_override_single_field() {
    let base = overlay::synthesize("m", "p");
    let ov: ModelOverride = serde_json::from_str(r#"{"speed": "fast"}"#).unwrap();
    let result = overlay::apply_override(base, &ov);

    assert_eq!(result.speed, SpeedTier::Fast);
    assert_eq!(result.cost, CostTier::Medium);
    assert_eq!(result.provider, "p");
}

#[test]
fn test_apply_override_all_fields() {
    let base = overlay::synthesize("m", "p");
    let ov = ModelOverride {
        provider: Some("custom".into()),
        display_name: Some("Custom Model".into()),
        context_window: Some(32_000),
        max_output_tokens: Some(4_096),
        speed: Some(SpeedTier::Slow),
        cost: Some(CostTier::Free),
        quality: Some(QualityTier::Premium),
        supports_tools: Some(false),
        supports_vision: Some(true),
        supports_prefill: Some(false),
        thinking: Some(ThinkingCapability::Adaptive),
    };
    let result = overlay::apply_override(base, &ov);

    assert_eq!(result.provider, "custom");
    assert_eq!(result.display_name, "Custom Model");
    assert_eq!(result.context_window, 32_000);
    assert_eq!(result.max_output_tokens, 4_096);
    assert_eq!(result.speed, SpeedTier::Slow);
    assert_eq!(result.cost, CostTier::Free);
    assert_eq!(result.quality, QualityTier::Premium);
    assert!(!result.supports_tools);
    assert!(result.supports_vision);
    assert!(!result.supports_prefill);
    assert_eq!(result.thinking, ThinkingCapability::Adaptive);
}

#[test]
fn test_apply_override_preserves_base_id() {
    let base = overlay::synthesize("original-id", "p");
    let ov: ModelOverride = serde_json::from_str(r#"{"provider": "new-p"}"#).unwrap();
    let result = overlay::apply_override(base, &ov);

    assert_eq!(result.id, "original-id");
    assert_eq!(result.display_name, "original-id");
    assert_eq!(result.provider, "new-p");
}
