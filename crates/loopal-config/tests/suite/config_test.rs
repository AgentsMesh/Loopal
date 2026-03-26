use loopal_config::{ProvidersConfig, Settings};
use loopal_tool_api::PermissionMode;

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
    assert_eq!(settings.permission_mode, PermissionMode::Bypass);
}

#[test]
fn test_settings_default_max_context_tokens() {
    let settings = Settings::default();
    assert_eq!(settings.max_context_tokens, 0);
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
    assert_eq!(settings.model, "claude-sonnet-4-20250514");
    assert_eq!(settings.max_turns, 50);
}

#[test]
fn test_settings_serde_partial_override() {
    let json = r#"{"model": "gpt-4", "max_turns": 100}"#;
    let settings: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(settings.model, "gpt-4");
    assert_eq!(settings.max_turns, 100);
    assert_eq!(settings.permission_mode, PermissionMode::Bypass);
    assert_eq!(settings.max_context_tokens, 0);
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

#[test]
fn test_mcp_server_config_map_format() {
    let json = r#"{
        "mcp_servers": {
            "github": {"command": "mcp-server-github", "args": ["--token", "abc"]},
            "sqlite": {"command": "mcp-sqlite", "enabled": false}
        }
    }"#;
    let settings: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(settings.mcp_servers.len(), 2);
    let github = settings.mcp_servers.get("github").unwrap();
    assert_eq!(github.command, "mcp-server-github");
    assert!(github.enabled);
    let sqlite = settings.mcp_servers.get("sqlite").unwrap();
    assert!(!sqlite.enabled);
}
