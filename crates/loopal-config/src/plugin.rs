use crate::layer::{ConfigLayer, LayerSource};
use crate::loader::load_layer_from_dir;
use crate::locations::global_plugins_dir;
use loopal_error::LoopalError;
use tracing::debug;

/// Scan `~/.loopal/plugins/` and load each subdirectory as a `ConfigLayer`.
///
/// Each plugin is a plain directory following the same isomorphic convention
/// as global/project layers. Plugins are sorted by name for deterministic
/// ordering, and all loaded with the lowest priority in the merge pipeline.
pub fn load_plugin_layers() -> Result<Vec<ConfigLayer>, LoopalError> {
    let plugins_dir = match global_plugins_dir() {
        Ok(d) => d,
        Err(_) => return Ok(Vec::new()),
    };

    if !plugins_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = std::fs::read_dir(&plugins_dir)?
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();

    // Sort by directory name for deterministic merge order
    entries.sort_by_key(|e| e.file_name());

    let mut layers = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let dir = entry.path();
        debug!(plugin = %name, "loading plugin layer");
        let layer = load_layer_from_dir(
            &dir,
            LayerSource::Plugin(name),
            None, // use <dir>/LOOPAL.md
        )?;
        layers.push(layer);
    }

    Ok(layers)
}
