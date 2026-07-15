use std::collections::HashSet;

use loopal_config::{LocalSettingsFieldPatch, OpenAiCompatConfig};
use loopal_ipc::protocol::methods::DesktopOpenAiCompatibleUpdate;
use serde_json::json;

use super::{safe_base_url, validate_env};
use crate::dispatch::registry::desktop_settings::validate_text;

pub(super) fn extend(
    updates: Vec<DesktopOpenAiCompatibleUpdate>,
    existing: &[OpenAiCompatConfig],
    fields: &mut Vec<LocalSettingsFieldPatch>,
) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }
    let mut providers = existing.to_vec();
    let mut names = HashSet::new();
    for update in updates {
        validate(&update)?;
        if !names.insert(update.name.clone()) {
            return Err(format!(
                "duplicate OpenAI-compatible update: {}",
                update.name
            ));
        }
        let index = providers.iter().position(|item| item.name == update.name);
        if update.remove {
            if let Some(index) = index {
                providers.remove(index);
            }
            continue;
        }
        if let Some(index) = index {
            apply(&mut providers[index], update);
        } else {
            let base_url = update
                .base_url
                .clone()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "new OpenAI-compatible provider requires baseUrl".to_string())?;
            let mut provider = OpenAiCompatConfig {
                name: update.name.clone(),
                base_url,
                api_key: None,
                api_key_env: None,
                model_prefix: None,
            };
            apply(&mut provider, update);
            providers.push(provider);
        }
    }
    fields.push(LocalSettingsFieldPatch::Set(
        "providers.openai_compat".into(),
        json!(providers),
    ));
    Ok(())
}

fn apply(provider: &mut OpenAiCompatConfig, update: DesktopOpenAiCompatibleUpdate) {
    if let Some(value) = update.base_url {
        provider.base_url = value;
    }
    if let Some(value) = update.api_key_env {
        provider.api_key_env = nonempty(value);
    }
    if let Some(value) = update.model_prefix {
        provider.model_prefix = nonempty(value);
    }
    if let Some(value) = update.api_key {
        provider.api_key = Some(value);
    } else if update.clear_api_key {
        provider.api_key = None;
    }
}

fn validate(update: &DesktopOpenAiCompatibleUpdate) -> Result<(), String> {
    validate_text("name", &update.name, 96, false)?;
    if update.remove
        && (update.base_url.is_some()
            || update.api_key_env.is_some()
            || update.model_prefix.is_some()
            || update.api_key.is_some()
            || update.clear_api_key)
    {
        return Err("compatible provider remove cannot include field changes".into());
    }
    if update.api_key.is_some() && update.clear_api_key {
        return Err("compatible provider apiKey cannot be set and cleared together".into());
    }
    if let Some(value) = &update.base_url {
        validate_text("baseUrl", value, 2048, false)?;
        if !safe_base_url(value) {
            return Err(
                "baseUrl must be a public http(s) URL without credentials, query, or fragment"
                    .into(),
            );
        }
    }
    if let Some(value) = &update.api_key_env {
        validate_env(value)?;
    }
    if let Some(value) = &update.model_prefix {
        validate_text("modelPrefix", value, 128, true)?;
    }
    if let Some(value) = &update.api_key {
        validate_text("apiKey", value, 8192, false)?;
    }
    Ok(())
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
