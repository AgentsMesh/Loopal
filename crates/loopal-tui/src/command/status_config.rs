use crate::app::ConfigEntry;

/// Serialize settings to JSON and recursively flatten to dot-notation key-value pairs.
pub(super) fn build_config_entries(settings: &loopal_config::Settings) -> Vec<ConfigEntry> {
    let value = match serde_json::to_value(settings) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut entries = Vec::new();
    flatten_json("", &value, &mut entries, 0);
    entries
}

/// Extract the primary provider's auth env var name and base URL.
/// Checks providers in priority order: Anthropic → OpenAI → Google.
pub(super) fn extract_provider_info(
    providers: &loopal_config::ProvidersConfig,
) -> (String, String) {
    let candidates = [
        (&providers.anthropic, "ANTHROPIC_API_KEY"),
        (&providers.openai, "OPENAI_API_KEY"),
        (&providers.google, "GOOGLE_API_KEY"),
    ];
    for (provider, default_env) in &candidates {
        if let Some(p) = provider {
            // Skip providers with no key configured at all.
            if p.api_key.is_none() && p.api_key_env.is_none() {
                continue;
            }
            let env = p.api_key_env.clone().unwrap_or_else(|| {
                if p.api_key.is_some() {
                    "(direct key)".to_string()
                } else {
                    (*default_env).to_string()
                }
            });
            let url = p.base_url.clone().unwrap_or_default();
            return (env, url);
        }
    }
    (String::new(), String::new())
}

const MAX_JSON_DEPTH: usize = 10;

/// Recursively flatten a JSON value into dot-notation `ConfigEntry` pairs.
/// Secrets (keys ending with "api_key") are redacted. Depth is bounded.
fn flatten_json(prefix: &str, value: &serde_json::Value, out: &mut Vec<ConfigEntry>, depth: usize) {
    if depth > MAX_JSON_DEPTH {
        out.push(ConfigEntry {
            key: prefix.to_string(),
            value: "(truncated)".to_string(),
        });
        return;
    }
    match value {
        serde_json::Value::Object(map) if map.is_empty() => {
            out.push(ConfigEntry {
                key: prefix.to_string(),
                value: "{}".to_string(),
            });
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_json(&key, v, out, depth + 1);
            }
        }
        serde_json::Value::Array(arr) if arr.is_empty() => {
            out.push(ConfigEntry {
                key: prefix.to_string(),
                value: "[]".to_string(),
            });
        }
        serde_json::Value::Array(arr) => {
            out.push(ConfigEntry {
                key: prefix.to_string(),
                value: format!("[{} items]", arr.len()),
            });
        }
        _ => {
            let is_secret = prefix
                .rsplit('.')
                .next()
                .is_some_and(|field| field == "api_key");
            let display = if is_secret && !value.is_null() {
                "********".to_string()
            } else {
                format_scalar(value)
            };
            out.push(ConfigEntry {
                key: prefix.to_string(),
                value: display,
            });
        }
    }
}

fn format_scalar(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

/// Overlay the live runtime values for the session-switchable settings onto
/// the disk-derived entries. Runtime `/permission`, `/decision`, `/sandbox`
/// switches are session-only (never persisted), so the disk config would
/// otherwise contradict what the pickers show as current. A differing live
/// value is marked `(session)`.
pub(super) fn overlay_runtime_values(
    entries: &mut [ConfigEntry],
    permission_mode: &str,
    decision_mode: &str,
    sandbox_policy: &str,
) {
    let live = [
        ("permission_mode", permission_mode),
        ("decision_mode", decision_mode),
        ("sandbox.policy", sandbox_policy),
    ];
    for e in entries.iter_mut() {
        if let Some((_, v)) = live.iter().find(|(k, _)| *k == e.key.as_str())
            && !v.is_empty()
            && *v != e.value
        {
            e.value = format!("{v} (session)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, value: &str) -> ConfigEntry {
        ConfigEntry {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn overlay_keys_match_real_settings_serialization() {
        // Pins the flattened serde keys against a real Settings: a rename or
        // nesting change fails here instead of silently no-op'ing the overlay.
        let settings = loopal_config::Settings {
            permission_mode: loopal_tool_api::PermissionMode::AskDangerous,
            sandbox: loopal_config::SandboxConfig {
                policy: loopal_config::SandboxPolicy::ReadOnly,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut entries = build_config_entries(&settings);
        // permission differs → marked; decision equals disk default → untouched.
        overlay_runtime_values(&mut entries, "bypass", "manual", "default_write");
        let get = |k: &str| {
            entries
                .iter()
                .find(|e| e.key == k)
                .map(|e| e.value.clone())
                .unwrap_or_else(|| panic!("missing flattened key {k}"))
        };
        assert_eq!(get("permission_mode"), "bypass (session)");
        assert_eq!(get("decision_mode"), "manual", "equal value not marked");
        assert_eq!(get("sandbox.policy"), "default_write (session)");
    }

    #[test]
    fn overlay_ignores_empty_live_values() {
        let mut entries = vec![entry("permission_mode", "bypass")];
        overlay_runtime_values(&mut entries, "", "", "");
        assert_eq!(entries[0].value, "bypass");
    }
}
