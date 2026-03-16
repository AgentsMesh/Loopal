use loopagent_provider::ProviderRegistry;
use loopagent_types::config::{ProviderConfig, ProvidersConfig, Settings};

#[test]
fn test_register_providers_google_with_config() {
    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: None,
        google: Some(ProviderConfig {
            api_key: Some("test-google-key-001".to_string()),
            api_key_env: None,
            base_url: None,
        }),
        openai_compat: vec![],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopagent_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("google").is_some(),
        "google provider should be registered with direct api_key"
    );
}

#[test]
fn test_register_providers_google_with_base_url() {
    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: None,
        google: Some(ProviderConfig {
            api_key: Some("test-google-key-base-url".to_string()),
            api_key_env: None,
            base_url: Some("https://custom-google.example.com".to_string()),
        }),
        openai_compat: vec![],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopagent_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("google").is_some(),
        "google provider should be registered with base_url"
    );
}

#[test]
fn test_register_providers_google_no_api_key_no_env() {
    let orig = std::env::var("GOOGLE_API_KEY").ok();
    unsafe {
        std::env::remove_var("GOOGLE_API_KEY");
    }

    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: None,
        google: Some(ProviderConfig {
            api_key: None,
            api_key_env: None,
            base_url: None,
        }),
        openai_compat: vec![],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopagent_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("google").is_none(),
        "google should NOT be registered without an API key"
    );

    unsafe {
        match orig {
            Some(v) => std::env::set_var("GOOGLE_API_KEY", v),
            None => std::env::remove_var("GOOGLE_API_KEY"),
        }
    }
}

#[test]
fn test_register_providers_google_no_base_url() {
    // Tests: google config exists but base_url is None
    let settings = Settings { providers: ProvidersConfig {
        anthropic: None,
        openai: None,
        google: Some(ProviderConfig {
            api_key: Some("test-google-no-base-url".to_string()),
            api_key_env: None,
            base_url: None, // No base_url
        }),
        openai_compat: vec![],
    }, ..Default::default() };

    let mut registry = ProviderRegistry::new();
    loopagent_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("google").is_some(),
        "google should be registered even without base_url"
    );
}
