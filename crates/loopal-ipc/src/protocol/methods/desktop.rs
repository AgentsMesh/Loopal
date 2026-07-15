use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::super::Method;

pub const DESKTOP_LIST_SESSIONS: Method = Method {
    name: "desktop/listSessions",
};

pub const DESKTOP_GET_SETTINGS: Method = Method {
    name: "desktop/getSettings",
};

pub const DESKTOP_UPDATE_SETTINGS: Method = Method {
    name: "desktop/updateSettings",
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopSettingsParams {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopSettingsValues {
    pub model: String,
    pub model_routing: DesktopModelRouting,
    pub permission_mode: String,
    pub decision_mode: String,
    pub sandbox_policy: String,
    pub thinking: Value,
    pub max_context_tokens: u32,
    pub memory_enabled: bool,
    pub microcompact_idle_minutes: u64,
    pub telemetry_enabled: bool,
    pub output_style: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopModelRouting {
    pub default: String,
    pub summarization: String,
    pub classification: String,
    pub refine: String,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopProviderUpdate {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopProviderUpdates {
    #[serde(default)]
    pub anthropic: Option<DesktopProviderUpdate>,
    #[serde(default)]
    pub openai: Option<DesktopProviderUpdate>,
    #[serde(default)]
    pub google: Option<DesktopProviderUpdate>,
    #[serde(default)]
    pub openai_compatible: Vec<DesktopOpenAiCompatibleUpdate>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopOpenAiCompatibleUpdate {
    pub name: String,
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub model_prefix: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopUpdateSettingsParams {
    pub workspace_id: String,
    pub settings: DesktopSettingsValues,
    #[serde(default)]
    pub provider_updates: DesktopProviderUpdates,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProviderSettings {
    pub enabled: bool,
    pub base_url: String,
    pub api_key_env: String,
    pub api_key_configured: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBuiltInProviders {
    pub anthropic: DesktopProviderSettings,
    pub openai: DesktopProviderSettings,
    pub google: DesktopProviderSettings,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopOpenAiCompatibleSettings {
    pub name: String,
    pub base_url: String,
    pub api_key_env: String,
    pub model_prefix: String,
    pub api_key_configured: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopResolvedSettingEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettingsResponse {
    pub workspace_id: String,
    pub settings: DesktopSettingsValues,
    pub configured_providers: Vec<String>,
    pub providers: DesktopBuiltInProviders,
    pub openai_compatible: Vec<DesktopOpenAiCompatibleSettings>,
    pub resolved_entries: Vec<DesktopResolvedSettingEntry>,
    pub setting_sources: Vec<String>,
}
