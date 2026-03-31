/// Known top-level keys in settings.json, derived from `Settings` struct fields.
const KNOWN_KEYS: &[&str] = &[
    "model",
    "model_routing",
    "models",
    "permission_mode",
    "max_context_tokens",
    "providers",
    "hooks",
    "mcp_servers",
    "sandbox",
    "thinking",
    "memory",
];

/// Log warnings for any unrecognised top-level keys in the merged config.
/// Called before `serde_json::from_value` so that typos (e.g. `"modle"`)
/// are surfaced instead of silently ignored.
pub fn warn_unknown_keys(merged: &serde_json::Value) {
    let obj = match merged.as_object() {
        Some(o) => o,
        None => return,
    };
    for key in obj.keys() {
        if !KNOWN_KEYS.contains(&key.as_str()) {
            tracing::warn!(key = %key, "unknown key in settings.json (typo?)");
        }
    }
}
