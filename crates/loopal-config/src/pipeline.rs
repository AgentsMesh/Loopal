//! Unified config pipeline — assembles layers and produces `ResolvedConfig`.

use std::path::Path;

use crate::layer::{ConfigLayer, LayerSource};
use crate::loader::{
    apply_env_overrides, extract_typed_fields, load_json_file, read_optional_text,
};
use crate::locations;
use crate::plugin::{load_plugin_layers, load_plugin_layers_from};
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
    let layers = load_config_layers(cwd)?;
    resolve(layers)
}

pub fn load_config_with_user_dir(
    cwd: &Path,
    user_dir: &Path,
) -> Result<ResolvedConfig, LoopalError> {
    resolve(load_config_layers_with_user_dir(cwd, user_dir)?)
}

pub fn load_user_config() -> Result<ResolvedConfig, LoopalError> {
    let dir = locations::global_config_dir()?;
    load_user_config_from_dir(&dir)
}

pub fn load_user_config_from_dir(user_dir: &Path) -> Result<ResolvedConfig, LoopalError> {
    resolve(load_user_layers(user_dir)?)
}

fn resolve(layers: Vec<ConfigLayer>) -> Result<ResolvedConfig, LoopalError> {
    let mut resolver = ConfigResolver::new();
    for layer in layers {
        resolver.add_layer(layer);
    }
    resolver.resolve()
}

pub fn load_config_layers(cwd: &Path) -> Result<Vec<ConfigLayer>, LoopalError> {
    let mut layers = load_plugin_layers()?;
    let global_dir = locations::global_config_dir().ok();
    if let Some(global_dir) = &global_dir {
        let instr_path = global_dir.join("LOOPAL.md");
        let layer =
            crate::loader::load_layer_from_dir(global_dir, LayerSource::Global, Some(&instr_path))?;
        layers.push(layer);
    }

    append_workspace_layers(&mut layers, cwd, global_dir.as_deref())?;
    Ok(layers)
}

fn load_config_layers_with_user_dir(
    cwd: &Path,
    user_dir: &Path,
) -> Result<Vec<ConfigLayer>, LoopalError> {
    let mut layers = load_user_layers(user_dir)?;
    append_workspace_layers(&mut layers, cwd, Some(user_dir))?;
    Ok(layers)
}

fn load_user_layers(user_dir: &Path) -> Result<Vec<ConfigLayer>, LoopalError> {
    let mut layers = load_plugin_layers_from(&user_dir.join("plugins"))?;
    let instructions = user_dir.join("LOOPAL.md");
    layers.push(crate::loader::load_layer_from_dir(
        user_dir,
        LayerSource::Global,
        Some(&instructions),
    )?);
    Ok(layers)
}

fn append_workspace_layers(
    layers: &mut Vec<ConfigLayer>,
    cwd: &Path,
    user_dir: Option<&Path>,
) -> Result<(), LoopalError> {
    let project_dir = locations::project_config_dir(cwd);
    let project_instr = locations::project_instructions_path(cwd);
    let layer = crate::loader::load_layer_from_dir(
        &project_dir,
        LayerSource::Project,
        Some(&project_instr),
    )?;
    layers.push(layer);
    layers.push(load_local_layer(cwd, user_dir)?);
    layers.push(load_env_layer());
    Ok(())
}

/// Load the Local override layer (settings.local.json + LOOPAL.local.md).
fn load_local_layer(cwd: &Path, user_dir: Option<&Path>) -> Result<ConfigLayer, LoopalError> {
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
    if let Some(text) = user_dir
        .map(|dir| dir.join("LOOPAL.local.md"))
        .as_deref()
        .and_then(read_optional_text)
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
