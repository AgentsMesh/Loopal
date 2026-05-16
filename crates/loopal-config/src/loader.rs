use std::path::Path;

use indexmap::IndexMap;

use crate::layer::{ConfigLayer, LayerSource};
use crate::loader_text::{TEXT_BYTE_LIMIT, read_text_bounded};
use crate::settings::McpServerConfig;
use crate::skills::scan_skills_dir;
use loopal_error::{ConfigError, LoopalError};

pub use crate::loader_env::apply_env_overrides;
pub(crate) use crate::loader_text::read_optional_text;

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

    if let Some(mcp_val) = value.get("mcp_servers")
        && let Some(obj) = mcp_val.as_object()
    {
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

/// Load a `ConfigLayer` from a directory following the isomorphic convention:
///
/// ```text
/// <dir>/
/// ├── settings.json     # settings + mcp_servers + hooks
/// ├── .mcp.json         # MCP servers (industry-standard format, overrides settings.json)
/// ├── skills/           # skill markdown files
/// └── LOOPAL.md         # instruction text
/// ```
pub fn load_layer_from_dir(
    dir: &Path,
    source: LayerSource,
    instructions_path: Option<&Path>,
) -> Result<ConfigLayer, LoopalError> {
    let mut layer = ConfigLayer {
        source,
        ..Default::default()
    };

    let mut settings_value = load_json_file(&dir.join("settings.json"))?;

    if !settings_value.is_null() {
        let (mcp, hooks) = extract_typed_fields(&mut settings_value);
        layer.mcp_servers = mcp;
        layer.hooks = hooks;
        layer.settings = settings_value;
    }

    let mcp_json = crate::mcp_json::load_mcp_json(&dir.join(".mcp.json"));
    for (name, config) in mcp_json {
        layer.mcp_servers.insert(name, config);
    }

    layer.skills = scan_skills_dir(&dir.join("skills"));

    let instr_path = instructions_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dir.join("LOOPAL.md"));
    layer.instructions = read_text_bounded(&instr_path, TEXT_BYTE_LIMIT);

    let memory_path = dir.join("memory").join("MEMORY.md");
    layer.memory = read_text_bounded(&memory_path, TEXT_BYTE_LIMIT);

    layer.classifier_prompt = read_text_bounded(&dir.join("classifier.md"), TEXT_BYTE_LIMIT);

    // vaults/ — directory containing N <name>.vault/ subdirs (path-only;
    // lazy decryption in runtime). For Project layer we walk up the tree so
    // a sub-agent with cwd_override into a sub-directory still resolves to
    // the parent's vaults.
    layer.vaults_dir = find_vaults_dir(dir);

    Ok(layer)
}

fn find_vaults_dir(start: &Path) -> Option<std::path::PathBuf> {
    let direct = start.join("vaults");
    if direct.is_dir() {
        return Some(direct);
    }
    // Walk up looking for `.loopal/vaults/` — Project layer's dir is
    // `<root>/.loopal`, so we ascend from its parent.
    let mut current = start.parent()?.parent()?;
    loop {
        let candidate = current.join(".loopal").join("vaults");
        if candidate.is_dir() {
            return Some(candidate);
        }
        current = match current.parent() {
            Some(p) => p,
            None => break,
        };
    }
    None
}
