use std::collections::HashMap;

use async_trait::async_trait;
use loopal_config::{OpenAiCompatConfig, ProviderConfig};
use loopal_secret_client::{IpcBudget, SecretClient, SecretResult, SecretString};

use super::*;

struct Secrets(HashMap<String, String>);

#[async_trait]
impl SecretClient for Secrets {
    async fn get(&self, name: &str, _: IpcBudget) -> SecretResult<SecretString> {
        self.0
            .get(name)
            .cloned()
            .map(SecretString::from)
            .ok_or_else(|| loopal_secret_client::SecretError::SecretNotFound(name.into()))
    }

    async fn list_names(&self, _: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(self.0.keys().cloned().collect())
    }

    async fn expand_author(&self, template: &str, _: IpcBudget) -> SecretResult<SecretString> {
        Ok(SecretString::from(template.to_string()))
    }

    async fn expand_wire(&self, template: &str, _: IpcBudget) -> SecretResult<SecretString> {
        Ok(SecretString::from(template.to_string()))
    }
}

fn provider(api_key: Option<&str>, base_url: Option<&str>) -> ProviderConfig {
    ProviderConfig {
        api_key: api_key.map(str::to_string),
        api_key_env: None,
        base_url: base_url.map(str::to_string),
    }
}

#[tokio::test]
async fn provider_secret_expansion_covers_native_and_compat_slots() {
    let mut settings = loopal_config::Settings::default();
    settings.providers.anthropic = Some(provider(
        Some("{{secret:anthropic_key}}"),
        Some("https://{{secret:host}}/anthropic"),
    ));
    settings.providers.openai = Some(provider(None, Some("https://plain.example")));
    settings.providers.google = None;
    settings.providers.openai_compat.push(OpenAiCompatConfig {
        name: "compat".into(),
        base_url: "https://{{secret:host}}/v1".into(),
        api_key: Some("{{secret:missing}}".into()),
        api_key_env: None,
        model_prefix: None,
    });
    settings.providers.openai_compat.push(OpenAiCompatConfig {
        name: "compat-without-key".into(),
        base_url: "https://{{secret:host}}/public".into(),
        api_key: None,
        api_key_env: None,
        model_prefix: None,
    });
    let client = Secrets(HashMap::from([
        ("anthropic_key".into(), "sk-secret".into()),
        ("host".into(), "api.example".into()),
    ]));

    expand_provider_secrets(&mut settings, &client).await;

    let anthropic = settings.providers.anthropic.unwrap();
    assert_eq!(anthropic.api_key.as_deref(), Some("sk-secret"));
    assert_eq!(
        anthropic.base_url.as_deref(),
        Some("https://api.example/anthropic")
    );
    assert_eq!(
        settings.providers.openai.unwrap().base_url.as_deref(),
        Some("https://plain.example")
    );
    let compat = &settings.providers.openai_compat[0];
    assert_eq!(compat.base_url, "https://api.example/v1");
    assert_eq!(compat.api_key.as_deref(), Some("<missing-secret:missing>"));
    let compat_without_key = &settings.providers.openai_compat[1];
    assert_eq!(compat_without_key.base_url, "https://api.example/public");
    assert_eq!(compat_without_key.api_key, None);
}
