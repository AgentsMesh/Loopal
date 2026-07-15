use std::path::Path;

use loopal_error::{ConfigError, LoopalError};

use crate::layer::LayerSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSummary {
    pub name: String,
    pub skills: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub hook_count: usize,
    pub has_instructions: bool,
    pub has_memory: bool,
    pub has_settings: bool,
}

pub fn list_plugins_from_user_dir(user_dir: &Path) -> Result<Vec<PluginSummary>, LoopalError> {
    let root = user_dir.join("plugins");
    if !root.exists() {
        return Ok(Vec::new());
    }
    if !root.is_dir() {
        return Err(ConfigError::InvalidValue {
            field: "plugins".into(),
            reason: "plugin inventory root must be a real directory".into(),
        }
        .into());
    }
    let mut entries = std::fs::read_dir(root)
        .map_err(LoopalError::Io)?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries
        .into_iter()
        .filter_map(|entry| summarize(&entry.path(), entry.file_name().to_str()?))
        .collect())
}

fn summarize(dir: &Path, name: &str) -> Option<PluginSummary> {
    if name.is_empty()
        || name.encode_utf16().count() > 128
        || name.contains('\\')
        || name.chars().any(char::is_control)
    {
        return None;
    }
    let layer =
        crate::loader::load_layer_from_dir(dir, LayerSource::Plugin(name.to_string()), None)
            .ok()?;
    let settings_path = dir.join("settings.json");
    let skills = layer
        .skills
        .into_iter()
        .map(|skill| skill.name)
        .filter(|name| skill_display_name(name))
        .collect();
    let mut mcp_servers = layer
        .mcp_servers
        .into_keys()
        .filter(|name| !name.is_empty() && name.encode_utf16().count() <= 128)
        .collect::<Vec<_>>();
    mcp_servers.sort();
    Some(PluginSummary {
        name: name.into(),
        skills,
        mcp_servers,
        hook_count: layer.hooks.len(),
        has_instructions: is_regular_file(&dir.join("LOOPAL.md")),
        has_memory: is_regular_file(&dir.join("memory/MEMORY.md")),
        has_settings: is_regular_file(&settings_path),
    })
}

fn skill_display_name(name: &str) -> bool {
    let Some(slug) = name.strip_prefix('/') else {
        return false;
    };
    !slug.is_empty()
        && name.encode_utf16().count() <= 256
        && !slug
            .chars()
            .any(|value| matches!(value, '/' | '\\') || value.is_control())
}

fn is_regular_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}
