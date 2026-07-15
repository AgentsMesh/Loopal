use std::path::Path;

use loopal_config::{
    CompactionSettings, LocalSettingsFieldPatch, Settings, patch_user_settings_fields,
};
use loopal_ipc::protocol::methods::{
    DesktopProviderUpdates, DesktopSettingsResponse, DesktopSettingsValues,
};
use serde_json::{Value, json};

mod projection;
mod provider_patch;

pub(super) fn load(
    root: &Path,
    user_dir: &Path,
    workspace_id: String,
) -> Result<DesktopSettingsResponse, String> {
    let user = loopal_config::load_user_config_from_dir(user_dir)
        .map_err(|_| "Loopal user configuration is invalid; repair ~/.loopal/settings.json")?;
    let effective = loopal_config::load_config_with_user_dir(root, user_dir).map_err(|_| {
        "Loopal configuration is invalid; repair the project settings files".to_string()
    })?;
    projection::response(workspace_id, &user, &effective)
}

pub(super) fn update(
    root: &Path,
    user_dir: &Path,
    workspace_id: String,
    values: DesktopSettingsValues,
    providers: DesktopProviderUpdates,
) -> Result<DesktopSettingsResponse, String> {
    let settings = validate(&values)?;
    let current = loopal_config::load_user_config_from_dir(user_dir)
        .map_err(|_| "Loopal user configuration is invalid; repair ~/.loopal/settings.json")?;
    let mut patches = fields(&settings, &values);
    provider_patch::extend(
        providers,
        &current.settings.providers.openai_compat,
        &mut patches,
    )?;
    patch_user_settings_fields(user_dir, patches).map_err(|error| error.to_string())?;
    load(root, user_dir, workspace_id)
}

fn validate(values: &DesktopSettingsValues) -> Result<Settings, String> {
    validate_text("model", &values.model, 256, false)?;
    validate_text("outputStyle", &values.output_style, 128, true)?;
    for (name, value) in routing_values(values) {
        validate_text(name, value, 256, true)?;
    }
    if values.thinking.get("type").and_then(Value::as_str) == Some("budget")
        && values.thinking.get("tokens").and_then(Value::as_u64) == Some(0)
    {
        return Err("thinking budget tokens must be greater than zero".into());
    }
    if values.microcompact_idle_minutes > CompactionSettings::MAX_MICROCOMPACT_IDLE_MINUTES {
        return Err(format!(
            "microcompactIdleMinutes must be at most {}",
            CompactionSettings::MAX_MICROCOMPACT_IDLE_MINUTES
        ));
    }
    let submitted_thinking = values.thinking.clone();
    let routes: serde_json::Map<String, Value> = routing_values(values)
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(name, value)| (name.into(), json!(value)))
        .collect();
    let settings: Settings = serde_json::from_value(json!({
        "model": values.model, "model_routing": routes,
        "permission_mode": values.permission_mode, "decision_mode": values.decision_mode,
        "sandbox": {"policy": values.sandbox_policy}, "thinking": values.thinking,
        "max_context_tokens": values.max_context_tokens,
        "memory": {"enabled": values.memory_enabled},
        "compaction": {"microcompact_idle_minutes": values.microcompact_idle_minutes},
        "telemetry": {"enabled": values.telemetry_enabled}, "output_style": values.output_style,
    }))
    .map_err(|error| format!("invalid Loopal settings: {error}"))?;
    if json!(settings.thinking) != submitted_thinking {
        return Err("thinking contains unsupported fields or values".into());
    }
    Ok(settings)
}

pub(super) fn validate_text(
    field: &str,
    value: &str,
    max: usize,
    allow_empty: bool,
) -> Result<(), String> {
    if (!allow_empty && value.trim().is_empty()) || value.len() > max {
        return Err(format!(
            "{field} must contain {} to {max} bytes",
            usize::from(!allow_empty)
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} must not contain control characters"));
    }
    Ok(())
}

fn fields(settings: &Settings, values: &DesktopSettingsValues) -> Vec<LocalSettingsFieldPatch> {
    use LocalSettingsFieldPatch::Set;
    let mut fields = vec![
        Set("model".into(), json!(settings.model)),
        Set("permission_mode".into(), json!(settings.permission_mode)),
        Set("decision_mode".into(), json!(settings.decision_mode)),
        Set("sandbox.policy".into(), json!(settings.sandbox.policy)),
        Set("thinking".into(), json!(settings.thinking)),
        Set(
            "max_context_tokens".into(),
            json!(settings.max_context_tokens),
        ),
        Set("memory.enabled".into(), json!(settings.memory.enabled)),
        Set(
            "compaction.microcompact_idle_minutes".into(),
            json!(settings.compaction.microcompact_idle_minutes),
        ),
        Set(
            "telemetry.enabled".into(),
            json!(settings.telemetry.enabled),
        ),
        Set("output_style".into(), json!(settings.output_style)),
    ];
    for (name, value) in routing_values(values) {
        let path = format!("model_routing.{name}");
        fields.push(if value.is_empty() {
            Set(path, Value::Null)
        } else {
            Set(path, json!(value))
        });
    }
    fields
}

fn routing_values(values: &DesktopSettingsValues) -> [(&'static str, &str); 4] {
    [
        ("default", &values.model_routing.default),
        ("summarization", &values.model_routing.summarization),
        ("classification", &values.model_routing.classification),
        ("refine", &values.model_routing.refine),
    ]
}
