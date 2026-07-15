use loopal_config::Settings;
use loopal_ipc::protocol::methods::DesktopResolvedSettingEntry;
use serde_json::Value;

use super::super::provider_patch::safe_base_url;

const MAX_DEPTH: usize = 10;
const MAX_ENTRIES: usize = 1024;
const MAX_VALUE_CHARS: usize = 512;

pub(super) fn resolved_entries(
    settings: &Settings,
) -> Result<Vec<DesktopResolvedSettingEntry>, String> {
    let value = serde_json::to_value(settings)
        .map_err(|error| format!("serialize resolved settings: {error}"))?;
    let mut entries = Vec::new();
    flatten("", &value, &mut entries, 0);
    Ok(entries)
}

pub(super) fn flatten(
    prefix: &str,
    value: &Value,
    out: &mut Vec<DesktopResolvedSettingEntry>,
    depth: usize,
) {
    if out.len() >= MAX_ENTRIES {
        return;
    }
    if sensitive(prefix) {
        push(out, prefix, "********".into());
    } else if depth > MAX_DEPTH {
        push(out, prefix, "(truncated)".into());
    } else {
        match value {
            Value::Object(map) if map.is_empty() => push(out, prefix, "{}".into()),
            Value::Object(map) => map.iter().for_each(|(key, value)| {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&path, value, out, depth + 1);
            }),
            Value::Array(items) => push(out, prefix, format!("[{} items]", items.len())),
            _ if url_key(prefix) && value.as_str().is_some_and(|url| !safe_base_url(url)) => {
                push(out, prefix, "********".into());
            }
            _ => push(out, prefix, scalar(value)),
        }
    }
}

fn sensitive(path: &str) -> bool {
    path.split('.').any(|part| {
        let key = part.to_ascii_lowercase().replace('-', "_");
        key == "env"
            || key.ends_with("_env")
            || key.contains("api_key")
            || key.contains("apikey")
            || key.contains("authorization")
            || key.contains("password")
            || key.contains("secret")
            || key == "headers"
            || key.ends_with("_headers")
            || key.contains("command")
            || key == "hooks"
            || key.starts_with("hook_")
            || key == "token"
            || key.ends_with("_token")
            || key.ends_with("_tokens")
    })
}

fn url_key(path: &str) -> bool {
    path.rsplit('.').next().is_some_and(|key| {
        key == "url" || key.ends_with("_url") || key == "endpoint" || key.ends_with("_endpoint")
    })
}

fn scalar(value: &Value) -> String {
    let text = match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    };
    text.chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(MAX_VALUE_CHARS)
        .collect()
}

fn push(out: &mut Vec<DesktopResolvedSettingEntry>, key: &str, value: String) {
    if out.len() < MAX_ENTRIES {
        out.push(DesktopResolvedSettingEntry {
            key: key
                .chars()
                .map(|character| {
                    if character.is_control() {
                        ' '
                    } else {
                        character
                    }
                })
                .take(512)
                .collect(),
            value,
        });
    }
}
