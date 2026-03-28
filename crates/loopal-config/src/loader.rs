use std::path::Path;

use indexmap::IndexMap;

use crate::layer::{ConfigLayer, LayerSource};
use crate::settings::McpServerConfig;
use crate::skills::scan_skills_dir;
use loopal_error::{ConfigError, LoopalError};

// ---------------------------------------------------------------------------
// Low-level helpers (public for unit tests)
// ---------------------------------------------------------------------------

/// Deep-merge two JSON values. Objects are merged recursively; all other types
/// (including arrays) are replaced by the overlay value.
pub fn deep_merge(base: &mut serde_json::Value, overlay: serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                deep_merge(
                    base_map.entry(key).or_insert(serde_json::Value::Null),
                    value,
                );
            }
        }
        (base, overlay) => {
            *base = overlay;
        }
    }
}

/// Load a JSON file and return its Value, or Null if the file does not exist.
pub fn load_json_file(path: &Path) -> Result<serde_json::Value, LoopalError> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let value: serde_json::Value = serde_json::from_str(&contents)
                .map_err(|e| ConfigError::Parse(format!("{}: {}", path.display(), e)))?;
            Ok(value)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::Value::Null),
        Err(e) => Err(LoopalError::Io(e)),
    }
}

/// Apply environment variable overrides to a JSON value.
pub fn apply_env_overrides(value: &mut serde_json::Value) {
    if !value.is_object() {
        *value = serde_json::json!({});
    }

    if let Ok(model) = std::env::var("LOOPAL_MODEL") {
        value["model"] = serde_json::Value::String(model);
    }

    if let Ok(max_turns) = std::env::var("LOOPAL_MAX_TURNS")
        && let Ok(n) = max_turns.parse::<u32>()
    {
        value["max_turns"] = serde_json::json!(n);
    }

    if let Ok(mode) = std::env::var("LOOPAL_PERMISSION_MODE") {
        value["permission_mode"] = serde_json::Value::String(mode);
    }

    if let Ok(sandbox) = std::env::var("LOOPAL_SANDBOX") {
        value["sandbox"]["policy"] = serde_json::Value::String(sandbox);
    }
}

// ---------------------------------------------------------------------------
// Helpers: extract typed fields from settings JSON
// ---------------------------------------------------------------------------

/// Extract `mcp_servers` and `hooks` from a settings JSON value into typed
/// fields, removing them from the raw value to avoid double-counting.
pub(crate) fn extract_typed_fields(
    value: &mut serde_json::Value,
) -> (
    IndexMap<String, McpServerConfig>,
    Vec<crate::hook::HookConfig>,
) {
    let mut mcp = IndexMap::new();
    let mut hooks = Vec::new();

    if let Some(mcp_val) = value.get("mcp_servers") {
        if let Some(obj) = mcp_val.as_object() {
            for (name, server_val) in obj {
                match serde_json::from_value::<McpServerConfig>(server_val.clone()) {
                    Ok(config) => {
                        mcp.insert(name.clone(), config);
                    }
                    Err(e) => {
                        tracing::warn!(server = %name, "invalid MCP server config, skipping: {e}");
                    }
                }
            }
        }
    }

    if let Some(hooks_val) = value.get("hooks") {
        match serde_json::from_value(hooks_val.clone()) {
            Ok(h) => hooks = h,
            Err(e) => tracing::warn!("invalid hooks config, skipping: {e}"),
        }
    }

    if let Some(obj) = value.as_object_mut() {
        obj.remove("mcp_servers");
        obj.remove("hooks");
    }

    (mcp, hooks)
}

/// Read optional text from a file path if it exists.
pub(crate) fn read_optional_text(path: &Path) -> Option<String> {
    if path.exists() {
        std::fs::read_to_string(path).ok()
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Isomorphic directory loader
// ---------------------------------------------------------------------------

/// Load a `ConfigLayer` from a directory following the isomorphic convention:
///
/// ```text
/// <dir>/
/// ├── settings.json     # settings + mcp_servers + hooks
/// ├── skills/           # skill markdown files
/// └── LOOPAL.md         # instruction text
/// ```
///
/// Missing files/directories are silently ignored.
pub fn load_layer_from_dir(
    dir: &Path,
    source: LayerSource,
    instructions_path: Option<&Path>,
) -> Result<ConfigLayer, LoopalError> {
    let mut layer = ConfigLayer {
        source,
        ..Default::default()
    };

    // settings.json — extract mcp_servers and hooks before storing raw JSON
    let mut settings_value = load_json_file(&dir.join("settings.json"))?;

    if !settings_value.is_null() {
        let (mcp, hooks) = extract_typed_fields(&mut settings_value);
        layer.mcp_servers = mcp;
        layer.hooks = hooks;
        layer.settings = settings_value;
    }

    // skills/ directory
    layer.skills = scan_skills_dir(&dir.join("skills"));

    // Instructions: use explicit path if given, otherwise <dir>/LOOPAL.md
    let instr_path = instructions_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dir.join("LOOPAL.md"));
    layer.instructions = read_optional_text(&instr_path);

    // memory/MEMORY.md — core memory file (agent writes via Write/Edit tools)
    let memory_path = dir.join("memory").join("MEMORY.md");
    layer.memory = read_optional_text(&memory_path);

    Ok(layer)
}
