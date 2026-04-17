use loopal_config::load_config;
use tempfile::TempDir;

#[test]
fn test_load_settings_all_env_var_scenarios() {
    // Combined test to avoid env var race conditions between parallel tests.
    // All subtests that touch LOOPAL_* env vars are serialized here.
    unsafe {
        std::env::remove_var("LOOPAL_MODEL");
        std::env::remove_var("LOOPAL_PERMISSION_MODE");
    }

    // --- Scenario 1: Defaults (no config files, no env vars) ---
    {
        let tmp = TempDir::new().unwrap();
        let settings = load_config(tmp.path()).unwrap().settings;
        assert_eq!(settings.model, "claude-opus-4-7");
        assert!(!settings.model.is_empty());
    }

    // --- Scenario 2: Project override ---
    {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".loopal");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("settings.json"), r#"{"model": "gpt-4"}"#).unwrap();

        let settings = load_config(tmp.path()).unwrap().settings;
        assert_eq!(settings.model, "gpt-4");
    }

    // --- Scenario 3: Env var overrides ---
    {
        unsafe {
            std::env::set_var("LOOPAL_MODEL", "test-model");
        }

        let tmp = TempDir::new().unwrap();
        let settings = load_config(tmp.path()).unwrap().settings;
        assert_eq!(settings.model, "test-model");

        unsafe {
            std::env::remove_var("LOOPAL_MODEL");
        }
    }

    // --- Scenario 4: Local settings override project ---
    {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".loopal");
        std::fs::create_dir_all(&config_dir).unwrap();

        std::fs::write(
            config_dir.join("settings.json"),
            r#"{"max_context_tokens": 100, "model": "gpt-4"}"#,
        )
        .unwrap();

        std::fs::write(
            config_dir.join("settings.local.json"),
            r#"{"max_context_tokens": 200}"#,
        )
        .unwrap();

        let settings = load_config(tmp.path()).unwrap().settings;
        assert_eq!(
            settings.max_context_tokens, 200,
            "local should override project"
        );
        assert_eq!(settings.model, "gpt-4", "model from project should persist");
    }

    // --- Scenario 5: LOOPAL_PERMISSION_MODE override ---
    {
        unsafe {
            std::env::set_var("LOOPAL_PERMISSION_MODE", "supervised");
        }

        let tmp = TempDir::new().unwrap();
        let settings = load_config(tmp.path()).unwrap().settings;
        assert_eq!(
            settings.permission_mode,
            loopal_tool_api::PermissionMode::Supervised,
            "env var should override permission mode"
        );

        unsafe {
            std::env::remove_var("LOOPAL_PERMISSION_MODE");
        }
    }

    // --- Scenario 6: LOOPAL_SANDBOX override ---
    {
        unsafe {
            std::env::set_var("LOOPAL_SANDBOX", "read_only");
        }

        let tmp = TempDir::new().unwrap();
        let settings = load_config(tmp.path()).unwrap().settings;
        assert_eq!(
            settings.sandbox.policy,
            loopal_config::SandboxPolicy::ReadOnly,
            "env var should override sandbox policy"
        );

        unsafe {
            std::env::remove_var("LOOPAL_SANDBOX");
        }
    }
}

#[test]
fn test_load_settings_deep_merge_nested_objects() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&config_dir).unwrap();

    std::fs::write(
        config_dir.join("settings.json"),
        r#"{
            "providers": {
                "anthropic": {
                    "api_key": "sk-proj-key",
                    "base_url": "https://api.anthropic.com"
                }
            }
        }"#,
    )
    .unwrap();

    std::fs::write(
        config_dir.join("settings.local.json"),
        r#"{
            "providers": {
                "anthropic": {
                    "api_key": "sk-local-key"
                }
            }
        }"#,
    )
    .unwrap();

    let settings = load_config(tmp.path()).unwrap().settings;
    let anthropic = settings.providers.anthropic.as_ref().unwrap();
    assert_eq!(
        anthropic.api_key.as_deref(),
        Some("sk-local-key"),
        "local override should replace the api_key"
    );
    assert_eq!(
        anthropic.base_url.as_deref(),
        Some("https://api.anthropic.com"),
        "base_url from project should persist through deep merge"
    );
}

#[test]
fn test_load_settings_invalid_json_returns_error() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("settings.json"), "{ invalid json }}").unwrap();

    let result = load_config(tmp.path());
    assert!(result.is_err(), "invalid JSON should produce an error");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Parse") || err.contains("parse") || err.contains("settings.json"));
}
