use loopal_config::{ProviderConfig, ProvidersConfig, Settings};
use loopal_mock_llm_lib::{Scenario, serve};
use loopal_provider::ProviderRegistry;
use loopal_provider_api::ChatParams;
use serde_json::json;
use tokio::net::TcpListener;

#[test]
fn test_register_providers_google_with_config() {
    let settings = Settings {
        providers: ProvidersConfig {
            anthropic: None,
            openai: None,
            google: Some(ProviderConfig {
                api_key: Some("test-google-key-001".to_string()),
                api_key_env: None,
                base_url: None,
            }),
            openai_compat: vec![],
        },
        ..Default::default()
    };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("google").is_some(),
        "google provider should be registered with direct api_key"
    );
}

#[test]
fn test_register_providers_google_with_base_url() {
    let settings = Settings {
        providers: ProvidersConfig {
            anthropic: None,
            openai: None,
            google: Some(ProviderConfig {
                api_key: Some("test-google-key-base-url".to_string()),
                api_key_env: None,
                base_url: Some("https://custom-google.example.com".to_string()),
            }),
            openai_compat: vec![],
        },
        ..Default::default()
    };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

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

    let settings = Settings {
        providers: ProvidersConfig {
            anthropic: None,
            openai: None,
            google: Some(ProviderConfig {
                api_key: None,
                api_key_env: None,
                base_url: None,
            }),
            openai_compat: vec![],
        },
        ..Default::default()
    };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

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
    let settings = Settings {
        providers: ProvidersConfig {
            anthropic: None,
            openai: None,
            google: Some(ProviderConfig {
                api_key: Some("test-google-no-base-url".to_string()),
                api_key_env: None,
                base_url: None, // No base_url
            }),
            openai_compat: vec![],
        },
        ..Default::default()
    };

    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);

    assert!(
        registry.get("google").is_some(),
        "google should be registered even without base_url"
    );
}

#[tokio::test]
async fn google_base_url_env_reaches_the_real_provider() {
    let scenario = Scenario::from_slice(
        &serde_json::to_vec(&json!({
            "version": 2,
            "calls": [{
                "expect": {"protocol": "google", "model": "env-model"},
                "chunks": [{"type": "done"}]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(serve(listener, scenario, "env-google-key".into()));
    let original = std::env::var("GOOGLE_BASE_URL").ok();
    unsafe { std::env::set_var("GOOGLE_BASE_URL", &base_url) };

    let settings = Settings {
        providers: ProvidersConfig {
            google: Some(ProviderConfig {
                api_key: Some("env-google-key".into()),
                api_key_env: None,
                base_url: None,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut registry = ProviderRegistry::new();
    loopal_kernel::register_providers(&settings, &mut registry);
    let provider = registry.get("google").unwrap();
    let result = provider
        .stream_chat(&ChatParams::new(
            "env-model".into(),
            Vec::new(),
            String::new(),
        ))
        .await;

    unsafe {
        match original {
            Some(value) => std::env::set_var("GOOGLE_BASE_URL", value),
            None => std::env::remove_var("GOOGLE_BASE_URL"),
        }
    }
    task.abort();
    assert!(result.is_ok());
}
