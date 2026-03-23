use loopal_config::{OpenAiCompatConfig, ProviderConfig, ProvidersConfig, Settings};
use loopal_provider::ProviderRegistry;

#[test]
fn test_register_providers_no_keys_no_crash() {
    // With no config keys, register_providers should not crash.
    let settings = Settings {
        providers: ProvidersConfig {
            anthropic: None,
            openai: None,
            google: None,
            openai_compat: vec![],
        },
        ..Default::default()
    };

    let mut registry = ProviderRegistry::new();
    // This should not panic regardless of what env vars are set
    loopal_kernel::register_providers(&settings, &mut registry);
}

#[test]
fn test_register_providers_no_config_no_env_vars() {
    // L149-154: all configs are None, all env vars cleared
    let orig_anthro_key = std::env::var("ANTHROPIC_API_KEY").ok();
    let orig_anthro_auth = std::env::var("ANTHROPIC_AUTH_TOKEN").ok();
    let orig_openai = std::env::var("OPENAI_API_KEY").ok();
    let orig_google = std::env::var("GOOGLE_API_KEY").ok();
    unsafe {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("GOOGLE_API_KEY");
    }

    let settings = Settings {
        providers: ProvidersConfig {
            anthropic: None,
            openai: None,
            google: None,
            openai_compat: vec![],
        },
        ..Default::default()
    };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);
    // Without any env vars or config, no providers should be registered
    assert!(registry.get("anthropic").is_none());
    assert!(registry.get("openai").is_none());
    assert!(registry.get("google").is_none());

    // Restore
    unsafe {
        match orig_anthro_key {
            Some(v) => std::env::set_var("ANTHROPIC_API_KEY", v),
            None => std::env::remove_var("ANTHROPIC_API_KEY"),
        }
        match orig_anthro_auth {
            Some(v) => std::env::set_var("ANTHROPIC_AUTH_TOKEN", v),
            None => std::env::remove_var("ANTHROPIC_AUTH_TOKEN"),
        }
        match orig_openai {
            Some(v) => std::env::set_var("OPENAI_API_KEY", v),
            None => std::env::remove_var("OPENAI_API_KEY"),
        }
        match orig_google {
            Some(v) => std::env::set_var("GOOGLE_API_KEY", v),
            None => std::env::remove_var("GOOGLE_API_KEY"),
        }
    }
}

#[test]
fn test_register_providers_multiple() {
    let settings = Settings {
        providers: ProvidersConfig {
            anthropic: Some(ProviderConfig {
                api_key: Some("multi-test-anthro-key".to_string()),
                api_key_env: None,
                base_url: None,
            }),
            openai: Some(ProviderConfig {
                api_key: Some("multi-test-openai-key".to_string()),
                api_key_env: None,
                base_url: Some("https://openai.example.com".to_string()),
            }),
            google: Some(ProviderConfig {
                api_key: Some("multi-test-google-key".to_string()),
                api_key_env: None,
                base_url: Some("https://google.example.com".to_string()),
            }),
            openai_compat: vec![OpenAiCompatConfig {
                name: "together".to_string(),
                base_url: "https://api.together.xyz/v1".to_string(),
                api_key: Some("multi-test-together-key".to_string()),
                api_key_env: None,
                model_prefix: None,
            }],
        },
        ..Default::default()
    };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("anthropic").is_some(),
        "anthropic should be registered"
    );
    assert!(
        registry.get("openai").is_some(),
        "openai should be registered"
    );
    assert!(
        registry.get("google").is_some(),
        "google should be registered"
    );
    assert!(
        registry.get("together").is_some(),
        "together (openai-compat) should be registered"
    );
}
