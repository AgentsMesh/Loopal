use super::provider_patch::{safe_base_url, safe_env_name};
use loopal_config::{LayerSource, OpenAiCompatConfig, ProviderConfig, ResolvedConfig, Settings};
use loopal_ipc::protocol::methods::{
    DesktopBuiltInProviders, DesktopModelRouting, DesktopOpenAiCompatibleSettings,
    DesktopProviderSettings, DesktopSettingsResponse, DesktopSettingsValues,
};
use serde_json::{Value, json};

mod resolved_entries;

#[cfg(test)]
use resolved_entries::flatten;
use resolved_entries::resolved_entries;

const DEFAULT_MODEL: &str = "claude-opus-4-8";

pub(super) fn response(
    workspace_id: String,
    user: &ResolvedConfig,
    effective: &ResolvedConfig,
) -> Result<DesktopSettingsResponse, String> {
    let settings = &user.settings;
    Ok(DesktopSettingsResponse {
        workspace_id,
        settings: values(settings),
        configured_providers: provider_names(settings),
        providers: DesktopBuiltInProviders {
            anthropic: provider(settings.providers.anthropic.as_ref()),
            openai: provider(settings.providers.openai.as_ref()),
            google: provider(settings.providers.google.as_ref()),
        },
        openai_compatible: settings
            .providers
            .openai_compat
            .iter()
            .take(61)
            .filter_map(compatible_provider)
            .collect(),
        resolved_entries: resolved_entries(&effective.settings)?,
        setting_sources: effective.layers.iter().take(32).map(source_label).collect(),
    })
}

fn compatible_provider(value: &OpenAiCompatConfig) -> Option<DesktopOpenAiCompatibleSettings> {
    let name = public_required_text(&value.name, 96, "");
    if name.is_empty() {
        return None;
    }
    let base_url = if safe_base_url(&value.base_url) {
        public_optional_text(&value.base_url, 2048)
    } else {
        String::new()
    };
    Some(DesktopOpenAiCompatibleSettings {
        name,
        base_url,
        api_key_env: value
            .api_key_env
            .as_deref()
            .filter(|name| name.len() <= 128 && safe_env_name(name))
            .unwrap_or_default()
            .to_string(),
        model_prefix: value
            .model_prefix
            .as_deref()
            .map(|value| public_optional_text(value, 128))
            .unwrap_or_default(),
        api_key_configured: value.api_key.as_ref().is_some_and(|key| !key.is_empty()),
    })
}

fn values(settings: &Settings) -> DesktopSettingsValues {
    let routes = json!(settings.model_routing);
    DesktopSettingsValues {
        model: public_required_text(&settings.model, 256, DEFAULT_MODEL),
        model_routing: DesktopModelRouting {
            default: route(&routes, "default"),
            summarization: route(&routes, "summarization"),
            classification: route(&routes, "classification"),
            refine: route(&routes, "refine"),
        },
        permission_mode: settings.permission_mode.to_string(),
        decision_mode: settings.decision_mode.to_string(),
        sandbox_policy: settings.sandbox.policy.to_string(),
        thinking: json!(settings.thinking),
        max_context_tokens: settings.max_context_tokens,
        memory_enabled: settings.memory.enabled,
        microcompact_idle_minutes: settings.compaction.microcompact_idle_minutes,
        telemetry_enabled: settings.telemetry.enabled,
        output_style: public_optional_text(&settings.output_style, 128),
    }
}

fn route(routes: &Value, name: &str) -> String {
    let value = routes.get(name).and_then(Value::as_str).unwrap_or_default();
    public_optional_text(value, 256)
}

fn provider(value: Option<&ProviderConfig>) -> DesktopProviderSettings {
    DesktopProviderSettings {
        enabled: value.is_some(),
        base_url: value
            .and_then(|item| item.base_url.as_deref())
            .filter(|url| url.len() <= 2048 && safe_base_url(url))
            .unwrap_or_default()
            .to_string(),
        api_key_env: value
            .and_then(|item| item.api_key_env.as_deref())
            .filter(|name| name.len() <= 128 && safe_env_name(name))
            .unwrap_or_default()
            .to_string(),
        api_key_configured: value
            .and_then(|item| item.api_key.as_ref())
            .is_some_and(|key| !key.is_empty()),
    }
}

fn provider_names(settings: &Settings) -> Vec<String> {
    let mut names = [
        ("anthropic", &settings.providers.anthropic),
        ("openai", &settings.providers.openai),
        ("google", &settings.providers.google),
    ]
    .into_iter()
    .filter(|(_, value)| value.is_some())
    .map(|(name, _)| name.to_string())
    .collect::<Vec<_>>();
    names.extend(
        settings
            .providers
            .openai_compat
            .iter()
            .take(61)
            .map(|item| {
                let name: String = item
                    .name
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(96)
                    .collect();
                format!("openai-compatible: {name}")
            }),
    );
    names
}

fn public_required_text(value: &str, max: usize, fallback: &str) -> String {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        fallback.into()
    } else {
        value.into()
    }
}

fn public_optional_text(value: &str, max: usize) -> String {
    if value.len() > max || value.chars().any(char::is_control) {
        String::new()
    } else {
        value.into()
    }
}

fn source_label(source: &LayerSource) -> String {
    match source {
        LayerSource::Global => "global settings".into(),
        LayerSource::Project => "project settings".into(),
        LayerSource::Local => "project local overrides".into(),
        LayerSource::Env => "environment overrides".into(),
        LayerSource::Cli => "CLI overrides".into(),
        LayerSource::Plugin(name) => {
            let safe: String = name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || "-_.".contains(*c))
                .take(64)
                .collect();
            if safe.is_empty() {
                "plugin settings".into()
            } else {
                format!("plugin:{safe}")
            }
        }
    }
}

#[cfg(test)]
mod tests;
