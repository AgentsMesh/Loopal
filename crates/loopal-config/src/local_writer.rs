use std::path::Path;

use loopal_error::{ConfigError, LoopalError};

use crate::loader::load_json_file;
use crate::locations;
pub use crate::settings_field_patch::LocalSettingsFieldPatch;
use crate::settings_file_lock::SettingsFileLock;

/// Set or overwrite a single top-level field in `<cwd>/.loopal/settings.local.json`,
/// preserving other fields. Creates the file and parent directory if missing.
///
/// The function operates on raw `serde_json::Value` rather than deserializing
/// into `Settings`. Settings uses `#[serde(default)]` on every field, so a
/// round-trip would inflate the file with every default value and erase the
/// override-only semantics of the Local layer.
pub fn update_local_settings_field(
    cwd: &Path,
    key: &str,
    value: serde_json::Value,
) -> Result<(), LoopalError> {
    update_local_settings_fields(cwd, [(key.to_string(), value)])
}

pub fn update_local_settings_fields(
    cwd: &Path,
    fields: impl IntoIterator<Item = (String, serde_json::Value)>,
) -> Result<(), LoopalError> {
    patch_local_settings_fields(
        cwd,
        fields
            .into_iter()
            .map(|(path, value)| LocalSettingsFieldPatch::Set(path, value)),
    )
}

pub fn patch_local_settings_fields(
    cwd: &Path,
    fields: impl IntoIterator<Item = LocalSettingsFieldPatch>,
) -> Result<(), LoopalError> {
    let path = locations::project_local_settings_path(cwd);
    let parent = path.parent().ok_or_else(|| ConfigError::InvalidValue {
        field: path.display().to_string(),
        reason: "path has no parent directory".into(),
    })?;
    std::fs::create_dir_all(parent).map_err(LoopalError::Io)?;
    let _lock = SettingsFileLock::acquire(parent.join(".settings.local.json.lock"))?;
    crate::local_gitignore::ensure_local_settings_ignored(parent)?;

    let mut current = load_json_file(&path)?;
    crate::settings_field_patch::apply(&mut current, fields)?;

    let serialized = serde_json::to_string_pretty(&current)
        .map_err(|e| ConfigError::Parse(format!("serialize settings.local.json: {e}")))?;
    crate::atomic_settings_write::atomic_write(&path, serialized.as_bytes())
}
