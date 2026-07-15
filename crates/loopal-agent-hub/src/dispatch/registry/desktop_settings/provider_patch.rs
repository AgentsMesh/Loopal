use loopal_config::{LocalSettingsFieldPatch, OpenAiCompatConfig};
use loopal_ipc::protocol::methods::{DesktopProviderUpdate, DesktopProviderUpdates};
use serde_json::{Value, json};

use super::validate_text;

mod openai_compatible;

pub(super) fn extend(
    updates: DesktopProviderUpdates,
    existing_compatible: &[OpenAiCompatConfig],
    fields: &mut Vec<LocalSettingsFieldPatch>,
) -> Result<(), String> {
    let compatible = updates.openai_compatible;
    for (name, update) in [
        ("anthropic", updates.anthropic),
        ("openai", updates.openai),
        ("google", updates.google),
    ] {
        if let Some(update) = update {
            provider(name, update, fields)?;
        }
    }
    openai_compatible::extend(compatible, existing_compatible, fields)?;
    Ok(())
}

fn provider(
    name: &str,
    update: DesktopProviderUpdate,
    fields: &mut Vec<LocalSettingsFieldPatch>,
) -> Result<(), String> {
    use LocalSettingsFieldPatch::{EnsureObject, Remove, Set};
    validate_update(name, &update)?;
    let root = format!("providers.{name}");
    if update.remove {
        fields.push(Remove(root));
        return Ok(());
    }
    if update.enabled == Some(false) {
        fields.push(Set(root, Value::Null));
        return Ok(());
    }
    if update.enabled == Some(true) {
        fields.push(EnsureObject(root.clone()));
    }
    if let Some(value) = update.base_url {
        fields.push(Set(format!("{root}.base_url"), optional_text(value)));
    }
    if let Some(value) = update.api_key_env {
        fields.push(Set(format!("{root}.api_key_env"), optional_text(value)));
    }
    if let Some(value) = update.api_key {
        fields.push(Set(format!("{root}.api_key"), json!(value)));
    } else if update.clear_api_key {
        fields.push(Set(format!("{root}.api_key"), Value::Null));
    }
    Ok(())
}

fn validate_update(name: &str, update: &DesktopProviderUpdate) -> Result<(), String> {
    let changes = update.base_url.is_some()
        || update.api_key_env.is_some()
        || update.api_key.is_some()
        || update.clear_api_key;
    if update.remove && (update.enabled.is_some() || changes) {
        return Err(format!(
            "{name} remove cannot be combined with other changes"
        ));
    }
    if update.enabled == Some(false) && changes {
        return Err(format!(
            "{name} disable cannot be combined with field changes"
        ));
    }
    if update.api_key.is_some() && update.clear_api_key {
        return Err(format!("{name} apiKey cannot be set and cleared together"));
    }
    if let Some(value) = &update.base_url {
        validate_text("baseUrl", value, 2048, true)?;
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
    if let Some(value) = &update.api_key {
        validate_text("apiKey", value, 8192, false)?;
    }
    Ok(())
}

pub(super) fn safe_base_url(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if value.contains('@')
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
    {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn validate_env(value: &str) -> Result<(), String> {
    validate_text("apiKeyEnv", value, 128, true)?;
    if !safe_env_name(value) {
        return Err("apiKeyEnv must be an environment variable name".into());
    }
    Ok(())
}

pub(super) fn safe_env_name(value: &str) -> bool {
    value.is_empty()
        || (!value.as_bytes()[0].is_ascii_digit()
            && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

fn optional_text(value: String) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        json!(value)
    }
}
