use loopal_provider::ProviderRegistry;
use loopal_types::config::{OpenAiCompatConfig, ProviderConfig, ProvidersConfig, Settings};

#[test]
fn test_register_providers_openai_with_config() {
    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: Some(ProviderConfig {
            api_key: Some("test-openai-key-001".to_string()),
            api_key_env: None,
            base_url: None,
        }),
        google: None,
        openai_compat: vec![],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("openai").is_some(),
        "openai provider should be registered with direct api_key"
    );
}

#[test]
fn test_register_providers_openai_with_base_url() {
    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: Some(ProviderConfig {
            api_key: Some("test-openai-key-base-url".to_string()),
            api_key_env: None,
            base_url: Some("https://custom-openai.example.com/v1".to_string()),
        }),
        google: None,
        openai_compat: vec![],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("openai").is_some(),
        "openai provider should be registered with base_url"
    );
}

#[test]
fn test_register_providers_openai_no_api_key_no_env() {
    let orig = std::env::var("OPENAI_API_KEY").ok();
    unsafe {
        std::env::remove_var("OPENAI_API_KEY");
    }

    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: Some(ProviderConfig {
            api_key: None,
            api_key_env: None,
            base_url: None,
        }),
        google: None,
        openai_compat: vec![],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("openai").is_none(),
        "openai should NOT be registered without an API key"
    );

    unsafe {
        match orig {
            Some(v) => std::env::set_var("OPENAI_API_KEY", v),
            None => std::env::remove_var("OPENAI_API_KEY"),
        }
    }
}

#[test]
fn test_register_providers_openai_no_base_url() {
    // Tests: openai config exists but base_url is None
    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: Some(ProviderConfig {
            api_key: Some("test-openai-no-base-url".to_string()),
            api_key_env: None,
            base_url: None, // No base_url
        }),
        google: None,
        openai_compat: vec![],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("openai").is_some(),
        "openai should be registered even without base_url"
    );
}

#[test]
fn test_register_providers_openai_compat() {
    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: None,
        google: None,
        openai_compat: vec![OpenAiCompatConfig {
            name: "ollama".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: Some("ollama-test-key".to_string()),
            api_key_env: None,
            model_prefix: Some("ollama/".to_string()),
        }],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("ollama").is_some(),
        "openai-compat 'ollama' provider should be registered"
    );
}

#[test]
fn test_register_providers_openai_compat_with_env_key() {
    let env_var = "LOOPAL_TEST_COMPAT_KEY_883271";
    unsafe {
        std::env::set_var(env_var, "compat-env-key-value");
    }

    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: None,
        google: None,
        openai_compat: vec![OpenAiCompatConfig {
            name: "test-compat-env".to_string(),
            base_url: "http://localhost:8080/v1".to_string(),
            api_key: None,
            api_key_env: Some(env_var.to_string()),
            model_prefix: None,
        }],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("test-compat-env").is_some(),
        "openai-compat provider should be registered via api_key_env"
    );

    unsafe {
        std::env::remove_var(env_var);
    }
}

#[test]
fn test_register_providers_openai_compat_no_key_skipped() {
    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: None,
        google: None,
        openai_compat: vec![OpenAiCompatConfig {
            name: "no-key-compat".to_string(),
            base_url: "http://localhost:9999/v1".to_string(),
            api_key: None,
            api_key_env: None,
            model_prefix: None,
        }],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("no-key-compat").is_none(),
        "openai-compat with no api_key should NOT be registered"
    );
}
