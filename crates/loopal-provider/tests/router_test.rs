use std::sync::Arc;

use async_trait::async_trait;
use loopal_provider::ProviderRegistry;
use loopal_types::error::LoopalError;
use loopal_types::provider::{ChatParams, ChatStream, Provider};

/// Minimal mock provider for testing the registry.
struct MockProvider {
    provider_name: String,
}

impl MockProvider {
    fn new(name: &str) -> Self {
        Self {
            provider_name: name.to_string(),
        }
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn stream_chat(&self, _params: &ChatParams) -> Result<ChatStream, LoopalError> {
        unimplemented!("mock provider does not support streaming")
    }
}

#[test]
fn test_register_and_resolve() {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(MockProvider::new("anthropic")));

    let provider = registry.resolve("claude-sonnet-4-20250514").unwrap();
    assert_eq!(provider.name(), "anthropic");
}

#[test]
fn test_resolve_openai_model() {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(MockProvider::new("openai")));

    let provider = registry.resolve("gpt-4o").unwrap();
    assert_eq!(provider.name(), "openai");
}

#[test]
fn test_resolve_google_model() {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(MockProvider::new("google")));

    let provider = registry.resolve("gemini-2.0-flash").unwrap();
    assert_eq!(provider.name(), "google");
}

#[test]
fn test_resolve_unknown_model_no_provider() {
    let registry = ProviderRegistry::new();
    let result = registry.resolve("claude-sonnet-4");
    assert!(result.is_err());
}

#[test]
fn test_resolve_unknown_fallback_to_openai_compat() {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(MockProvider::new("openai_compat")));

    let provider = registry.resolve("llama-3").unwrap();
    assert_eq!(provider.name(), "openai_compat");
}

#[test]
fn test_get_by_name() {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(MockProvider::new("anthropic")));

    assert!(registry.get("anthropic").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_default_registry_is_empty() {
    let registry = ProviderRegistry::default();
    assert!(registry.get("anthropic").is_none());
    assert!(registry.resolve("gpt-4o").is_err());
}

#[test]
fn test_register_overwrites_existing() {
    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(MockProvider::new("anthropic")));
    registry.register(Arc::new(MockProvider::new("anthropic")));

    // Should still resolve fine (last registration wins)
    let provider = registry.resolve("claude-sonnet-4").unwrap();
    assert_eq!(provider.name(), "anthropic");
}
