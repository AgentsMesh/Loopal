use loopal_config::{ImageSettings, Settings};

#[test]
fn image_settings_default_matches_spec() {
    let s = ImageSettings::default();
    assert_eq!(s.max_bytes, 10 * 1024 * 1024);
    assert_eq!(s.max_pixels, 8192 * 8192);
    assert_eq!(s.inline_threshold_bytes, 256 * 1024);
}

#[test]
fn settings_uses_default_images_when_absent() {
    let json = r#"{"model": "claude-test"}"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(s.images.max_bytes, 10 * 1024 * 1024);
    assert_eq!(s.images.inline_threshold_bytes, 256 * 1024);
}

#[test]
fn settings_deserializes_explicit_image_policy() {
    let json = r#"{
        "model": "claude-test",
        "images": {
            "max_bytes": 5000000,
            "max_pixels": 4000000,
            "inline_threshold_bytes": 65536
        }
    }"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(s.images.max_bytes, 5000000);
    assert_eq!(s.images.max_pixels, 4000000);
    assert_eq!(s.images.inline_threshold_bytes, 65536);
}

#[test]
fn settings_image_partial_uses_defaults_for_missing_fields() {
    let json = r#"{
        "model": "claude-test",
        "images": {"inline_threshold_bytes": 1024}
    }"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(s.images.inline_threshold_bytes, 1024);
    assert_eq!(s.images.max_bytes, 10 * 1024 * 1024);
    assert_eq!(s.images.max_pixels, 8192 * 8192);
}

#[test]
fn image_settings_round_trips_through_json() {
    let original = ImageSettings {
        max_bytes: 1,
        max_pixels: 2,
        inline_threshold_bytes: 3,
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: ImageSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(original.max_bytes, back.max_bytes);
    assert_eq!(original.max_pixels, back.max_pixels);
    assert_eq!(original.inline_threshold_bytes, back.inline_threshold_bytes);
}
