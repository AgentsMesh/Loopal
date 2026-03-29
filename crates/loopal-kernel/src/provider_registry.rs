use std::sync::Arc;

use loopal_config::Settings;
use loopal_provider::{
    AnthropicProvider, GoogleProvider, OpenAiCompatProvider, OpenAiProvider, ProviderRegistry,
};
use tracing::info;

/// Register all configured providers into the given registry.
pub fn register_providers(settings: &Settings, registry: &mut ProviderRegistry) {
    // Initialize user model metadata overlay before any provider resolution.
    loopal_provider::init_user_models(&settings.models);

    let providers = &settings.providers;

    // Anthropic — explicit config or auto-detect from env
    let anthropic_key = providers
        .anthropic
        .as_ref()
        .and_then(|c| resolve_api_key(&c.api_key, &c.api_key_env))
        .or_else(|| {
            std::env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|k| !k.is_empty())
        })
        .or_else(|| {
            std::env::var("ANTHROPIC_AUTH_TOKEN")
                .ok()
                .filter(|k| !k.is_empty())
        });

    if let Some(api_key) = anthropic_key {
        let mut provider = AnthropicProvider::new(api_key);
        // Base URL: config > env var
        let base_url = providers
            .anthropic
            .as_ref()
            .and_then(|c| c.base_url.clone())
            .or_else(|| {
                std::env::var("ANTHROPIC_BASE_URL")
                    .ok()
                    .filter(|u| !u.is_empty())
            });
        if let Some(url) = base_url {
            provider = provider.with_base_url(url);
        }
        registry.register(Arc::new(provider));
        info!("registered anthropic provider");
    }

    // OpenAI — explicit config or auto-detect from env
    let openai_key = providers
        .openai
        .as_ref()
        .and_then(|c| resolve_api_key(&c.api_key, &c.api_key_env))
        .or_else(|| {
            std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|k| !k.is_empty())
        });

    if let Some(api_key) = openai_key {
        let mut provider = OpenAiProvider::new(api_key);
        if let Some(ref config) = providers.openai
            && let Some(ref base_url) = config.base_url
        {
            provider = provider.with_base_url(base_url.clone());
        }
        registry.register(Arc::new(provider));
        info!("registered openai provider");
    }

    // Google — explicit config or auto-detect from env
    let google_key = providers
        .google
        .as_ref()
        .and_then(|c| resolve_api_key(&c.api_key, &c.api_key_env))
        .or_else(|| {
            std::env::var("GOOGLE_API_KEY")
                .ok()
                .filter(|k| !k.is_empty())
        });

    if let Some(api_key) = google_key {
        let mut provider = GoogleProvider::new(api_key);
        if let Some(ref config) = providers.google
            && let Some(ref base_url) = config.base_url
        {
            provider = provider.with_base_url(base_url.clone());
        }
        registry.register(Arc::new(provider));
        info!("registered google provider");
    }

    // OpenAI-compatible providers
    for compat in &providers.openai_compat {
        if let Some(api_key) = resolve_api_key(&compat.api_key, &compat.api_key_env) {
            let provider =
                OpenAiCompatProvider::new(api_key, compat.base_url.clone(), compat.name.clone());
            if let Some(ref prefix) = compat.model_prefix {
                registry.register_with_prefix(Arc::new(provider), prefix);
                info!(name = %compat.name, prefix, "registered openai-compat provider (prefix)");
            } else {
                registry.register(Arc::new(provider));
                info!(name = %compat.name, "registered openai-compat provider");
            }
        }
    }
}

/// Resolve an API key from a direct value or an environment variable.
/// Direct key takes precedence over the environment variable.
pub fn resolve_api_key(api_key: &Option<String>, api_key_env: &Option<String>) -> Option<String> {
    // Direct key takes precedence
    if let Some(key) = api_key
        && !key.is_empty()
    {
        return Some(key.clone());
    }
    // Try environment variable
    if let Some(env_var) = api_key_env
        && let Ok(key) = std::env::var(env_var)
        && !key.is_empty()
    {
        return Some(key);
    }
    None
}
