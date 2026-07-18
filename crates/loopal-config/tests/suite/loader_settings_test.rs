use loopal_config::load_config;
use tempfile::TempDir;

#[test]
fn test_load_settings_all_env_var_scenarios() {
    // Combined test to avoid env var race conditions between parallel tests.
    // All subtests that touch LOOPAL_* env vars are serialized here.
    // reason: HOME is isolated too — load_config merges the GLOBAL layer from
    // ~/.loopal, so a developer machine with a real settings.json there would
    // otherwise leak its model into the "defaults" scenario.
    let isolated_home = TempDir::new().unwrap();
    let original_home = std::env::var_os("HOME");
    let original_profile = std::env::var_os("USERPROFILE");
    unsafe {
        std::env::set_var("HOME", isolated_home.path());
        std::env::set_var("USERPROFILE", isolated_home.path());
        std::env::remove_var("LOOPAL_MODEL");
        std::env::remove_var("LOOPAL_PERMISSION_MODE");
        std::env::remove_var("LOOPAL_DECISION_MODE");
    }
    let restore_home = scopeguard(move || unsafe {
        match &original_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match &original_profile {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
    });
    let _restore_home = &restore_home;

    // --- Scenario 1: Defaults (no config files, no env vars) ---
    {
        let tmp = TempDir::new().unwrap();
        let settings = load_config(tmp.path()).unwrap().settings;
        assert_eq!(settings.model, "claude-opus-4-8");
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
            std::env::set_var("LOOPAL_PERMISSION_MODE", "ask_any_write");
        }

        let tmp = TempDir::new().unwrap();
        let settings = load_config(tmp.path()).unwrap().settings;
        assert_eq!(
            settings.permission_mode,
            loopal_tool_api::PermissionMode::AskAnyWrite,
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

    // --- Scenario 7: LOOPAL_DECISION_MODE override ---
    {
        unsafe {
            std::env::set_var("LOOPAL_DECISION_MODE", "classifier");
        }

        let tmp = TempDir::new().unwrap();
        let settings = load_config(tmp.path()).unwrap().settings;
        assert_eq!(
            settings.decision_mode,
            loopal_decision_api::DecisionMode::Classifier,
            "env var should override decision mode"
        );

        unsafe {
            std::env::remove_var("LOOPAL_DECISION_MODE");
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

struct EnvRestore<F: FnMut()>(F);

impl<F: FnMut()> Drop for EnvRestore<F> {
    fn drop(&mut self) {
        (self.0)()
    }
}

fn scopeguard<F: FnMut()>(restore: F) -> EnvRestore<F> {
    EnvRestore(restore)
}
