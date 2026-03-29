use loopal_config::Settings;
use loopal_provider_api::TaskType;

#[test]
fn test_settings_model_routing_serde() {
    let json = r#"{
        "model": "claude-sonnet-4-6",
        "model_routing": {
            "summarization": "claude-haiku-3-5-20241022"
        }
    }"#;
    let settings: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(settings.model, "claude-sonnet-4-6");
    assert_eq!(
        settings
            .model_routing
            .get(&TaskType::Summarization)
            .unwrap(),
        "claude-haiku-3-5-20241022"
    );
    assert!(!settings.model_routing.contains_key(&TaskType::Default));
}

#[test]
fn test_settings_models_override_serde() {
    let json = r#"{
        "model": "claude-sonnet-4-6",
        "models": {
            "my-llama": {
                "provider": "ollama",
                "context_window": 32000,
                "speed": "fast",
                "cost": "free"
            }
        }
    }"#;
    let settings: Settings = serde_json::from_str(json).unwrap();
    let ov = settings.models.get("my-llama").unwrap();
    assert_eq!(ov.provider.as_deref(), Some("ollama"));
    assert_eq!(ov.context_window, Some(32000));
}

#[test]
fn test_settings_default_has_no_routing() {
    let settings = Settings::default();
    assert!(settings.model_routing.is_empty());
    assert!(settings.models.is_empty());
}

#[test]
fn test_settings_roundtrip_with_routing() {
    let json = r#"{
        "model": "gpt-4o",
        "model_routing": {
            "summarization": "gpt-4o-mini",
            "default": "gpt-4o"
        }
    }"#;
    let settings: Settings = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_string(&settings).unwrap();
    let back: Settings = serde_json::from_str(&serialized).unwrap();
    assert_eq!(back.model_routing.len(), 2);
    assert_eq!(
        back.model_routing.get(&TaskType::Default).unwrap(),
        "gpt-4o"
    );
}
