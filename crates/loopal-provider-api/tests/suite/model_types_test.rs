use loopal_provider_api::{
    CostTier, ModelOverride, QualityTier, SpeedTier, TaskType, ThinkingCapability,
};

// -- TaskType serde --------------------------------------------------------

#[test]
fn test_task_type_serialize_snake_case() {
    let json = serde_json::to_string(&TaskType::Summarization).unwrap();
    assert_eq!(json, "\"summarization\"");

    let json = serde_json::to_string(&TaskType::Default).unwrap();
    assert_eq!(json, "\"default\"");
}

#[test]
fn test_task_type_deserialize_snake_case() {
    let t: TaskType = serde_json::from_str("\"summarization\"").unwrap();
    assert_eq!(t, TaskType::Summarization);

    let t: TaskType = serde_json::from_str("\"default\"").unwrap();
    assert_eq!(t, TaskType::Default);
}

// -- Tier enums serde ------------------------------------------------------

#[test]
fn test_speed_tier_serde_roundtrip() {
    for tier in [SpeedTier::Fast, SpeedTier::Medium, SpeedTier::Slow] {
        let json = serde_json::to_string(&tier).unwrap();
        let back: SpeedTier = serde_json::from_str(&json).unwrap();
        assert_eq!(tier, back);
    }
}

#[test]
fn test_cost_tier_serde_roundtrip() {
    for tier in [
        CostTier::Free,
        CostTier::Low,
        CostTier::Medium,
        CostTier::High,
    ] {
        let json = serde_json::to_string(&tier).unwrap();
        let back: CostTier = serde_json::from_str(&json).unwrap();
        assert_eq!(tier, back);
    }
}

#[test]
fn test_quality_tier_serde_roundtrip() {
    for tier in [
        QualityTier::Basic,
        QualityTier::Standard,
        QualityTier::Premium,
    ] {
        let json = serde_json::to_string(&tier).unwrap();
        let back: QualityTier = serde_json::from_str(&json).unwrap();
        assert_eq!(tier, back);
    }
}

// -- ModelOverride serde ---------------------------------------------------

#[test]
fn test_model_override_partial_fields() {
    let json = r#"{"provider":"ollama","speed":"fast","supports_tools":false}"#;
    let ov: ModelOverride = serde_json::from_str(json).unwrap();
    assert_eq!(ov.provider.as_deref(), Some("ollama"));
    assert_eq!(ov.speed, Some(SpeedTier::Fast));
    assert_eq!(ov.supports_tools, Some(false));
    assert!(ov.context_window.is_none());
    assert!(ov.thinking.is_none());
}

#[test]
fn test_model_override_all_fields() {
    let json = r#"{
        "provider": "custom",
        "display_name": "My Custom Model",
        "context_window": 32000,
        "max_output_tokens": 4096,
        "speed": "slow",
        "cost": "free",
        "quality": "premium",
        "supports_tools": true,
        "supports_vision": true,
        "thinking": "Adaptive"
    }"#;
    let ov: ModelOverride = serde_json::from_str(json).unwrap();
    assert_eq!(ov.display_name.as_deref(), Some("My Custom Model"));
    assert_eq!(ov.cost, Some(CostTier::Free));
    assert_eq!(ov.quality, Some(QualityTier::Premium));
    assert_eq!(ov.thinking, Some(ThinkingCapability::Adaptive));
    assert_eq!(ov.context_window, Some(32000));
}
