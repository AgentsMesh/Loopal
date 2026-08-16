pub async fn expand_provider_secrets(
    settings: &mut loopal_config::Settings,
    client: &dyn loopal_secret_client::SecretClient,
) {
    let budget = loopal_ipc::HUB_RPC_BUDGET;
    for slot in [
        &mut settings.providers.anthropic,
        &mut settings.providers.openai,
        &mut settings.providers.google,
    ] {
        if let Some(config) = slot.as_mut() {
            if let Some(api_key) = config.api_key.as_mut() {
                *api_key =
                    loopal_secret_runtime::expand_to_plaintext(api_key, client, budget).await;
            }
            if let Some(base_url) = config.base_url.as_mut() {
                *base_url =
                    loopal_secret_runtime::expand_to_plaintext(base_url, client, budget).await;
            }
        }
    }
    for config in &mut settings.providers.openai_compat {
        config.base_url =
            loopal_secret_runtime::expand_to_plaintext(&config.base_url, client, budget).await;
        if let Some(api_key) = config.api_key.as_mut() {
            *api_key = loopal_secret_runtime::expand_to_plaintext(api_key, client, budget).await;
        }
    }
}

#[cfg(test)]
#[path = "provider_secrets/tests.rs"]
mod tests;
