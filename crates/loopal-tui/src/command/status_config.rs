//! Config data collection for the `/status` sub-page.
//!
//! Serializes `Settings` to JSON, recursively flattens to dot-notation entries,
//! and extracts provider auth/URL info.

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

// ---------------------------------------------------------------------------
// JSON flattening
// ---------------------------------------------------------------------------

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
