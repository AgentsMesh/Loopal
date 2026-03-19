use loopal_config::load_settings;
use tempfile::TempDir;

#[test]
fn test_load_settings_all_env_var_scenarios() {
    // Combined test to avoid env var race conditions between parallel tests.
    // All subtests that touch LOOPAL_* env vars are serialized here.
    unsafe {
        std::env::remove_var("LOOPAL_MODEL");
        std::env::remove_var("LOOPAL_MAX_TURNS");
        std::env::remove_var("LOOPAL_PERMISSION_MODE");
    }

    // --- Scenario 1: Defaults (no config files, no env vars) ---
    {
        let tmp = TempDir::new().unwrap();
        let settings = load_settings(tmp.path()).unwrap();
        assert_eq!(settings.max_turns, 50);
        assert_eq!(settings.model, "claude-sonnet-4-20250514");
        assert!(!settings.model.is_empty());
        assert!(settings.max_turns > 0);
    }

    // --- Scenario 2: Project override ---
    {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".loopal");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("settings.json"),
            r#"{"max_turns": 100, "model": "gpt-4"}"#,
        )
        .unwrap();

        let settings = load_settings(tmp.path()).unwrap();
        assert_eq!(settings.max_turns, 100);
        assert_eq!(settings.model, "gpt-4");
    }

    // --- Scenario 3: Env var overrides ---
    {
        unsafe {
            std::env::set_var("LOOPAL_MODEL", "test-model");
            std::env::set_var("LOOPAL_MAX_TURNS", "10");
        }

        let tmp = TempDir::new().unwrap();
        let settings = load_settings(tmp.path()).unwrap();
        assert_eq!(settings.model, "test-model");
        assert_eq!(settings.max_turns, 10);

        unsafe {
            std::env::remove_var("LOOPAL_MODEL");
            std::env::remove_var("LOOPAL_MAX_TURNS");
        }
    }

    // --- Scenario 4: Local settings override project ---
    {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".loopal");
        std::fs::create_dir_all(&config_dir).unwrap();

        std::fs::write(
            config_dir.join("settings.json"),
            r#"{"max_turns": 100, "model": "gpt-4"}"#,
        )
        .unwrap();

        std::fs::write(
            config_dir.join("settings.local.json"),
            r#"{"max_turns": 200}"#,
        )
        .unwrap();

        let settings = load_settings(tmp.path()).unwrap();
        assert_eq!(settings.max_turns, 200, "local should override project");
        assert_eq!(settings.model, "gpt-4", "model from project should persist");
    }

    // --- Scenario 5: LOOPAL_PERMISSION_MODE override ---
    {
        unsafe {
            std::env::set_var("LOOPAL_PERMISSION_MODE", "Supervised");
        }

        let tmp = TempDir::new().unwrap();
        let settings = load_settings(tmp.path()).unwrap();
        assert_eq!(
            settings.permission_mode,
            loopal_types::permission::PermissionMode::Supervised,
            "env var should override permission mode"
        );

        unsafe {
            std::env::remove_var("LOOPAL_PERMISSION_MODE");
        }
    }

    // --- Scenario 6: Non-numeric LOOPAL_MAX_TURNS is ignored ---
    {
        unsafe {
            std::env::set_var("LOOPAL_MAX_TURNS", "not_a_number");
        }

        let tmp = TempDir::new().unwrap();
        let settings = load_settings(tmp.path()).unwrap();
        assert_eq!(settings.max_turns, 50, "non-numeric max_turns should be ignored");

        unsafe {
            std::env::remove_var("LOOPAL_MAX_TURNS");
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

    let settings = load_settings(tmp.path()).unwrap();
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

    std::fs::write(
        config_dir.join("settings.json"),
        "{ this is not valid json }}}",
    )
    .unwrap();

    let result = load_settings(tmp.path());
    assert!(result.is_err(), "invalid JSON should produce an error");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Parse") || err_msg.contains("parse") || err_msg.contains("settings.json"),
        "error should mention parsing, got: {}",
        err_msg
    );
}

