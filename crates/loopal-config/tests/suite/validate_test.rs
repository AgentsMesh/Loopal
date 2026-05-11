use std::collections::HashSet;

use loopal_config::Settings;

#[test]
fn known_keys_matches_settings_struct() {
    let serialized = serde_json::to_value(Settings::default()).unwrap();
    let actual_keys: HashSet<String> = serialized
        .as_object()
        .expect("Settings serializes to object")
        .keys()
        .cloned()
        .collect();
    let declared: HashSet<String> = loopal_config::known_keys()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let missing: Vec<&String> = actual_keys.difference(&declared).collect();
    assert!(
        missing.is_empty(),
        "KNOWN_KEYS is missing fields present in Settings: {missing:?}. \
         Add them to validate.rs::KNOWN_KEYS or users will see false 'unknown key' warnings."
    );
    let stale: Vec<&String> = declared.difference(&actual_keys).collect();
    assert!(
        stale.is_empty(),
        "KNOWN_KEYS lists fields not present in Settings: {stale:?}. \
         Remove them from validate.rs::KNOWN_KEYS."
    );
}
