//! Unified config pipeline — assembles layers and produces `ResolvedConfig`.

use std::path::Path;

use crate::layer::{ConfigLayer, LayerSource};
use crate::loader::{
    apply_env_overrides, extract_typed_fields, load_json_file, read_optional_text,
};
use crate::locations;
use crate::plugin::load_plugin_layers;
use crate::resolved::ResolvedConfig;
use crate::resolver::ConfigResolver;
use loopal_error::LoopalError;

/// Load and merge all configuration layers into a single `ResolvedConfig`.
///
/// Layer priority (lowest → highest):
/// 1. Plugin layers (~/.loopal/plugins/<name>/)
/// 2. Global layer (~/.loopal/)
/// 3. Project layer (<cwd>/.loopal/)
/// 4. Local overrides (settings.local.json + LOOPAL.local.md)
/// 5. Environment variable overrides
pub fn load_config(cwd: &Path) -> Result<ResolvedConfig, LoopalError> {
    let mut resolver = ConfigResolver::new();

    // 1. Plugin layers (lowest priority, sorted by name)
    for layer in load_plugin_layers()? {
        resolver.add_layer(layer);
    }

    // 2. Global layer — isomorphic load
    if let Ok(global_dir) = locations::global_config_dir() {
        let instr_path = global_dir.join("LOOPAL.md");
        let layer = crate::loader::load_layer_from_dir(
            &global_dir,
            LayerSource::Global,
            Some(&instr_path),
        )?;
        resolver.add_layer(layer);
    }

    // 3. Project layer — isomorphic load
    let project_dir = locations::project_config_dir(cwd);
    let project_instr = locations::project_instructions_path(cwd);
    let layer = crate::loader::load_layer_from_dir(
        &project_dir,
        LayerSource::Project,
        Some(&project_instr),
    )?;
    resolver.add_layer(layer);

    // 4. Local overrides
    resolver.add_layer(load_local_layer(cwd)?);

    // 5. Environment overrides
    resolver.add_layer(load_env_layer());

    resolver.resolve()
}

/// Load the Local override layer (settings.local.json + LOOPAL.local.md).
fn load_local_layer(cwd: &Path) -> Result<ConfigLayer, LoopalError> {
    let mut layer = ConfigLayer {
        source: LayerSource::Local,
        ..Default::default()
    };

    let mut settings_value = load_json_file(&locations::project_local_settings_path(cwd))?;
    if !settings_value.is_null() {
        let (mcp, hooks) = extract_typed_fields(&mut settings_value);
        layer.mcp_servers = mcp;
        layer.hooks = hooks;
        layer.settings = settings_value;
    }

    // LOOPAL.local.md — global then project
    let mut parts = Vec::new();
    if let Ok(global_local) = locations::global_local_instructions_path()
        && let Some(text) = read_optional_text(&global_local)
    {
        parts.push(text);
    }
    if let Some(text) = read_optional_text(&locations::project_local_instructions_path(cwd)) {
        parts.push(text);
    }
    if !parts.is_empty() {
        layer.instructions = Some(parts.join("\n\n"));
    }

    Ok(layer)
}

/// Build a layer from environment variable overrides.
fn load_env_layer() -> ConfigLayer {
    let mut value = serde_json::json!({});
    apply_env_overrides(&mut value);
    ConfigLayer {
        source: LayerSource::Env,
        settings: value,
        ..Default::default()
    }
}
