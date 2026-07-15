use loopal_error::{ConfigError, LoopalError};

use crate::loader::load_json_file;
use crate::settings_field_patch::LocalSettingsFieldPatch;
use crate::settings_file_lock::SettingsFileLock;

pub fn patch_global_settings_fields(
    fields: impl IntoIterator<Item = LocalSettingsFieldPatch>,
) -> Result<(), LoopalError> {
    let dir = crate::locations::global_config_dir()?;
    patch_user_settings_fields(&dir, fields)
}

pub fn patch_user_settings_fields(
    user_dir: &std::path::Path,
    fields: impl IntoIterator<Item = LocalSettingsFieldPatch>,
) -> Result<(), LoopalError> {
    let path = user_dir.join("settings.json");
    let parent = path.parent().ok_or_else(|| ConfigError::InvalidValue {
        field: path.display().to_string(),
        reason: "path has no parent directory".into(),
    })?;
    std::fs::create_dir_all(parent).map_err(LoopalError::Io)?;
    let _lock = SettingsFileLock::acquire(parent.join(".settings.json.lock"))?;
    let mut current = load_json_file(&path)?;
    crate::settings_field_patch::apply(&mut current, fields)?;
    let serialized = serde_json::to_string_pretty(&current)
        .map_err(|error| ConfigError::Parse(format!("serialize settings.json: {error}")))?;
    crate::atomic_settings_write::atomic_write(&path, serialized.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalSettingsFieldPatch::{Remove, Set};
    use serde_json::json;

    #[test]
    fn user_writer_is_private_atomic_and_preserves_unknown_fields() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings.json");
        std::fs::write(&path, r#"{"unknown":{"keep":true},"remove":"value"}"#).unwrap();
        patch_user_settings_fields(
            root.path(),
            [
                Set("providers.openai.api_key".into(), json!("secret")),
                Remove("remove".into()),
            ],
        )
        .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["unknown"]["keep"], true);
        assert_eq!(value["providers"]["openai"]["api_key"], "secret");
        assert!(value.get("remove").is_none());
        assert!(!root.path().join(".gitignore").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
