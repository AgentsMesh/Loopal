use std::path::Path;

use loopal_types::config::Settings;
use loopal_types::error::{ConfigError, LoopalError};

use crate::locations;

/// Deep-merge two JSON values. Objects are merged recursively; all other types
/// (including arrays) are replaced by the overlay value.
fn deep_merge(base: &mut serde_json::Value, overlay: serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                deep_merge(base_map.entry(key).or_insert(serde_json::Value::Null), value);
            }
        }
        (base, overlay) => {
            *base = overlay;
        }
    }
}

/// Load a JSON file and return its Value, or Null if the file does not exist.
fn load_json_file(path: &Path) -> Result<serde_json::Value, LoopalError> {
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
fn apply_env_overrides(value: &mut serde_json::Value) {
    // Ensure we have an object to work with
    if !value.is_object() {
        *value = serde_json::json!({});
    }

    if let Ok(model) = std::env::var("LOOPAL_MODEL") {
        value["model"] = serde_json::Value::String(model);
    }

    if let Ok(max_turns) = std::env::var("LOOPAL_MAX_TURNS")
        && let Ok(n) = max_turns.parse::<u32>() {
            value["max_turns"] = serde_json::json!(n);
        }

    if let Ok(mode) = std::env::var("LOOPAL_PERMISSION_MODE") {
        value["permission_mode"] = serde_json::Value::String(mode);
    }

    if let Ok(sandbox) = std::env::var("LOOPAL_SANDBOX") {
        value["sandbox"]["policy"] = serde_json::Value::String(sandbox);
    }
}

/// Load settings with 5-layer merge:
/// 1. Defaults (from Settings::default())
/// 2. Global settings.json
/// 3. Project settings.json
/// 4. Project settings.local.json
/// 5. Environment variable overrides
pub fn load_settings(cwd: &Path) -> Result<Settings, LoopalError> {
    // Start with defaults serialized to Value
    let mut merged = serde_json::to_value(Settings::default())
        .map_err(|e| ConfigError::Parse(e.to_string()))?;

    // Layer 2: global settings
    let global = load_json_file(&locations::global_settings_path()?)?;
    if !global.is_null() {
        deep_merge(&mut merged, global);
    }

    // Layer 3: project settings
    let project = load_json_file(&locations::project_settings_path(cwd))?;
    if !project.is_null() {
        deep_merge(&mut merged, project);
    }

    // Layer 4: project local settings
    let local = load_json_file(&locations::project_local_settings_path(cwd))?;
    if !local.is_null() {
        deep_merge(&mut merged, local);
    }

    // Layer 5: environment overrides
    apply_env_overrides(&mut merged);

    // Warn about unrecognised keys before deserialising
    crate::validate::warn_unknown_keys(&merged);

    let settings: Settings = serde_json::from_value(merged)
        .map_err(|e| ConfigError::Parse(e.to_string()))?;

    Ok(settings)
}

/// Load and concatenate instruction files (LOOPAL.md).
/// Global instructions come first, then project instructions, separated by newlines.
pub fn load_instructions(cwd: &Path) -> Result<String, LoopalError> {
    let mut parts = Vec::new();

    let global_path = locations::global_instructions_path()?;
    if global_path.exists() {
        parts.push(std::fs::read_to_string(&global_path)?);
    }

    let project_path = locations::project_instructions_path(cwd);
    if project_path.exists() {
        parts.push(std::fs::read_to_string(&project_path)?);
    }

    Ok(parts.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_merge_replaces_non_object() {
        // L17: base is not Object, overlay replaces it entirely
        let mut base = serde_json::json!("a string");
        let overlay = serde_json::json!("replaced");
        deep_merge(&mut base, overlay);
        assert_eq!(base, serde_json::json!("replaced"));
    }

    #[test]
    fn test_deep_merge_objects_recursive() {
        let mut base = serde_json::json!({"a": {"b": 1, "c": 2}});
        let overlay = serde_json::json!({"a": {"b": 10}});
        deep_merge(&mut base, overlay);
        assert_eq!(base["a"]["b"], 10);
        assert_eq!(base["a"]["c"], 2);
    }

    #[test]
    fn test_deep_merge_object_replaces_non_object_at_key() {
        let mut base = serde_json::json!({"key": "string_value"});
        let overlay = serde_json::json!({"key": {"nested": true}});
        deep_merge(&mut base, overlay);
        assert_eq!(base["key"]["nested"], true);
    }

    #[test]
    fn test_load_json_file_not_found_returns_null() {
        // L31: NotFound returns Ok(Null)
        let path = std::path::Path::new("/tmp/loopal_test_nonexistent_file_xyz_12345.json");
        let result = load_json_file(path).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_load_json_file_valid_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("test.json");
        std::fs::write(&file, r#"{"key": "value"}"#).unwrap();

        let result = load_json_file(&file).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_load_json_file_invalid_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("bad.json");
        std::fs::write(&file, "not valid json!").unwrap();

        let result = load_json_file(&file);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_json_file_io_error() {
        // L32: IO error that is NOT NotFound
        // Reading a directory path will cause an IO error
        let tmp = tempfile::TempDir::new().unwrap();
        let result = load_json_file(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_env_overrides_on_non_object() {
        // L39-40: value is not an object, should be replaced with {}
        let mut value = serde_json::json!("a string");
        apply_env_overrides(&mut value);
        assert!(value.is_object());
    }

    #[test]
    fn test_apply_env_overrides_on_object() {
        // L39: value is already an object
        let mut value = serde_json::json!({"existing": true});
        apply_env_overrides(&mut value);
        assert!(value.is_object());
        assert_eq!(value["existing"], true);
    }
}