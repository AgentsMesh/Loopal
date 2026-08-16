use loopal_tool_api::OneShotChatError;

#[test]
fn settings_default_threshold_is_8kb() {
    let config = loopal_config::FetchRefinerConfig::default();
    assert_eq!(config.threshold_bytes, 8 * 1024);
    assert!(config.enabled);
}

#[test]
fn settings_does_not_pin_a_specific_model() {
    let config = loopal_config::FetchRefinerConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    assert!(
        !json.contains("model"),
        "model should be sourced from model_routing[refine], not embedded — got {json}"
    );
}

#[test]
fn one_shot_chat_error_displays_distinct_messages() {
    let messages: Vec<String> = [
        OneShotChatError::Timeout,
        OneShotChatError::ProviderUnresolvable,
        OneShotChatError::StreamFailed,
        OneShotChatError::ChunkFailed,
        OneShotChatError::EmptyResponse,
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let unique: std::collections::HashSet<&String> = messages.iter().collect();
    assert_eq!(unique.len(), messages.len());
}
