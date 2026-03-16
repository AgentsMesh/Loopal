use loopagent_types::config::{ProvidersConfig, Settings};
use loopagent_types::permission::PermissionMode;

#[test]
fn test_settings_default_model() {
    let settings = Settings::default();
    assert_eq!(settings.model, "claude-sonnet-4-20250514");
}

#[test]
fn test_settings_default_max_turns() {
    let settings = Settings::default();
    assert_eq!(settings.max_turns, 50);
}

#[test]
fn test_settings_default_permission_mode() {
    let settings = Settings::default();
    assert_eq!(settings.permission_mode, PermissionMode::BypassPermissions);
}

#[test]
fn test_settings_default_max_context_tokens() {
    let settings = Settings::default();
    assert_eq!(settings.max_context_tokens, 200_000);
}

#[test]
fn test_settings_default_max_cost_none() {
    let settings = Settings::default();
    assert!(settings.max_cost.is_none());
}

#[test]
fn test_settings_default_hooks_empty() {
    let settings = Settings::default();
    assert!(settings.hooks.is_empty());
}

#[test]
fn test_settings_default_mcp_servers_empty() {
    let settings = Settings::default();
    assert!(settings.mcp_servers.is_empty());
}

#[test]
fn test_settings_serde_roundtrip() {
    let settings = Settings::default();
    let json = serde_json::to_string(&settings).unwrap();
    let deserialized: Settings = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.model, settings.model);
    assert_eq!(deserialized.max_turns, settings.max_turns);
    assert_eq!(deserialized.permission_mode, settings.permission_mode);
    assert_eq!(deserialized.max_context_tokens, settings.max_context_tokens);
    assert_eq!(deserialized.max_cost, settings.max_cost);
    assert_eq!(deserialized.hooks.len(), settings.hooks.len());
    assert_eq!(deserialized.mcp_servers.len(), settings.mcp_servers.len());
}

#[test]
fn test_settings_serde_from_empty_json() {
    let json = "{}";
    let settings: Settings = serde_json::from_str(json).unwrap();
    // Should produce defaults
    assert_eq!(settings.model, "claude-sonnet-4-20250514");
    assert_eq!(settings.max_turns, 50);
}

#[test]
fn test_settings_serde_partial_override() {
    let json = r#"{"model": "gpt-4", "max_turns": 100}"#;
    let settings: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(settings.model, "gpt-4");
    assert_eq!(settings.max_turns, 100);
    // Remaining fields should be defaults
    assert_eq!(settings.permission_mode, PermissionMode::BypassPermissions);
    assert_eq!(settings.max_context_tokens, 200_000);
}

#[test]
fn test_providers_config_default_all_none() {
    let providers = ProvidersConfig::default();
    assert!(providers.anthropic.is_none());
    assert!(providers.openai.is_none());
    assert!(providers.google.is_none());
    assert!(providers.openai_compat.is_empty());
}

#[test]
fn test_providers_config_serde_roundtrip() {
    let providers = ProvidersConfig::default();
    let json = serde_json::to_string(&providers).unwrap();
    let deserialized: ProvidersConfig = serde_json::from_str(&json).unwrap();
    assert!(deserialized.anthropic.is_none());
    assert!(deserialized.openai.is_none());
    assert!(deserialized.google.is_none());
    assert!(deserialized.openai_compat.is_empty());
}
