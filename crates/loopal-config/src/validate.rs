const KNOWN_KEYS: &[&str] = &[
    "model",
    "model_routing",
    "models",
    "permission_mode",
    "decision_mode",
    "max_context_tokens",
    "providers",
    "hooks",
    "mcp_servers",
    "sandbox",
    "thinking",
    "memory",
    "harness",
    "output_style",
    "telemetry",
    "fetch_refiner",
    "secrets",
    "goals",
    "compaction",
    "bg_tasks",
];

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

#[doc(hidden)]
pub fn known_keys() -> &'static [&'static str] {
    KNOWN_KEYS
}
