use std::path::Path;

use loopal_error::{ConfigError, LoopalError};

use crate::loader::load_json_file;
use crate::locations;

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
    let path = locations::project_local_settings_path(cwd);

    let mut current = load_json_file(&path)?;
    if current.is_null() {
        current = serde_json::Value::Object(serde_json::Map::new());
    }
    let obj = current.as_object_mut().ok_or_else(|| {
        ConfigError::InvalidValue {
            field: path.display().to_string(),
            reason: "top-level JSON is not an object".into(),
        }
    })?;
    obj.insert(key.to_string(), value);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(LoopalError::Io)?;
    }
    let serialized = serde_json::to_string_pretty(&current)
        .map_err(|e| ConfigError::Parse(format!("serialize settings.local.json: {e}")))?;
    atomic_write(&path, serialized.as_bytes())
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<(), LoopalError> {
    use std::io::Write;
    let parent = path.parent().ok_or_else(|| {
        ConfigError::InvalidValue {
            field: path.display().to_string(),
            reason: "path has no parent directory".into(),
        }
    })?;
    let tmp = parent.join(format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("settings.local.json"),
        std::process::id()
    ));
    {
        let mut f = std::fs::File::create(&tmp).map_err(LoopalError::Io)?;
        f.write_all(data).map_err(LoopalError::Io)?;
        f.sync_all().map_err(LoopalError::Io)?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        LoopalError::Io(e)
    })?;
    Ok(())
}
