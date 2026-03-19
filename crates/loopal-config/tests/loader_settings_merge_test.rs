use loopal_config::load_settings;
use tempfile::TempDir;

#[test]
fn test_load_settings_openai_compat_config() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&config_dir).unwrap();

    std::fs::write(
        config_dir.join("settings.json"),
        r#"{
            "providers": {
                "openai_compat": [
                    {
                        "name": "ollama",
                        "base_url": "http://localhost:11434/v1",
                        "api_key": "test-key"
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    let settings = load_settings(tmp.path()).unwrap();
    assert_eq!(settings.providers.openai_compat.len(), 1);
    assert_eq!(settings.providers.openai_compat[0].name, "ollama");
    assert_eq!(
        settings.providers.openai_compat[0].base_url,
        "http://localhost:11434/v1"
    );
}

#[test]
fn test_load_settings_local_overrides_project_deep_nested() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&config_dir).unwrap();

    std::fs::write(
        config_dir.join("settings.json"),
        r#"{
            "max_turns": 30,
            "providers": {
                "openai": {
                    "api_key": "proj-openai-key",
                    "base_url": "https://proj-openai.com"
                },
                "google": {
                    "api_key": "proj-google-key"
                }
            }
        }"#,
    )
    .unwrap();

    std::fs::write(
        config_dir.join("settings.local.json"),
        r#"{
            "max_turns": 75,
            "providers": {
                "openai": {
                    "api_key": "local-openai-key"
                }
            }
        }"#,
    )
    .unwrap();

    let settings = load_settings(tmp.path()).unwrap();
    assert_eq!(settings.max_turns, 75, "local should override max_turns");

    let openai = settings.providers.openai.as_ref().unwrap();
    assert_eq!(
        openai.api_key.as_deref(),
        Some("local-openai-key"),
        "local should override openai api_key"
    );
    assert_eq!(
        openai.base_url.as_deref(),
        Some("https://proj-openai.com"),
        "base_url from project should persist through deep merge"
    );

    let google = settings.providers.google.as_ref().unwrap();
    assert_eq!(
        google.api_key.as_deref(),
        Some("proj-google-key"),
        "google config from project should persist when local doesn't override it"
    );
}

#[test]
fn test_load_settings_mcp_servers_config() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&config_dir).unwrap();

    std::fs::write(
        config_dir.join("settings.json"),
        r#"{
            "mcp_servers": [
                {
                    "name": "test-mcp",
                    "command": "node",
                    "args": ["server.js"],
                    "env": {"PORT": "3000"}
                }
            ]
        }"#,
    )
    .unwrap();

    let settings = load_settings(tmp.path()).unwrap();
    assert_eq!(settings.mcp_servers.len(), 1);
    assert_eq!(settings.mcp_servers[0].name, "test-mcp");
    assert_eq!(settings.mcp_servers[0].command, "node");
    assert_eq!(settings.mcp_servers[0].args, vec!["server.js"]);
    assert_eq!(settings.mcp_servers[0].env.get("PORT").unwrap(), "3000");
}

#[test]
fn test_load_settings_empty_json_file_uses_defaults() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&config_dir).unwrap();

    std::fs::write(config_dir.join("settings.json"), "{}").unwrap();

    let settings = load_settings(tmp.path()).unwrap();
    assert_eq!(settings.max_turns, 50);
    assert_eq!(settings.model, "claude-sonnet-4-20250514");
}

#[test]
fn test_load_settings_invalid_local_json_returns_error() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&config_dir).unwrap();

    std::fs::write(config_dir.join("settings.json"), r#"{"max_turns": 10}"#).unwrap();
    std::fs::write(config_dir.join("settings.local.json"), "NOT_JSON!!").unwrap();

    let result = load_settings(tmp.path());
    assert!(result.is_err(), "invalid local JSON should produce an error");
}

#[test]
fn test_load_settings_hooks_config() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&config_dir).unwrap();

    std::fs::write(
        config_dir.join("settings.json"),
        r#"{
            "hooks": [
                {
                    "event": "pre_tool_use",
                    "command": "echo hook running",
                    "timeout_ms": 5000
                }
            ]
        }"#,
    )
    .unwrap();

    let settings = load_settings(tmp.path()).unwrap();
    assert_eq!(settings.hooks.len(), 1);
    assert_eq!(settings.hooks[0].command, "echo hook running");
}
