use super::*;

#[test]
fn projection_bounds_and_redacts_nested_secrets_and_url_like_endpoints() {
    let long_key = "x".repeat(900);
    let long_value = "y".repeat(900);
    let marker = "projection-secret-marker";
    let mut map = serde_json::Map::new();
    map.insert(long_key, json!(long_value));
    map.insert("headers".into(), json!({"Authorization": marker}));
    map.insert(
        "callback_url".into(),
        json!(format!(
            "https://user:{marker}@example.test/?token={marker}"
        )),
    );
    map.insert(
        "telemetry.otlp_endpoint".into(),
        json!(format!("https://example.test/otlp?authorization={marker}")),
    );
    let value = Value::Object(map);
    let mut entries = Vec::new();
    flatten("", &value, &mut entries, 0);
    assert!(entries.iter().all(|entry| entry.key.chars().count() <= 512));
    assert!(
        entries
            .iter()
            .all(|entry| entry.value.chars().count() <= 512)
    );
    for key in ["headers", "callback_url", "telemetry.otlp_endpoint"] {
        assert!(
            entries
                .iter()
                .any(|entry| entry.key == key && entry.value == "********")
        );
    }
    assert!(!entries.iter().any(|entry| entry.value.contains(marker)));
}

#[test]
fn source_labels_and_base_url_validation_are_bounded_and_safe() {
    let source = source_label(&LayerSource::Plugin(format!("../../{}", "z".repeat(300))));
    assert!(source.len() <= 71);
    assert!(!source.contains('/'));
    assert!(!safe_base_url("https://[::::]/v1"));
    assert!(!safe_base_url("https://example.test:99999/v1"));
    assert!(!safe_base_url("https://example.test/\0secret"));
    assert!(safe_base_url("https://[::1]:8443/v1"));
}

#[test]
fn legacy_text_values_cannot_poison_the_strict_desktop_contract() {
    let settings: Settings = serde_json::from_value(json!({
        "model": format!("bad\0{}", "m".repeat(300)),
        "model_routing": {
            "default": format!("{}", "r".repeat(300)),
            "summarization": "bad\nroute"
        },
        "output_style": format!("bad\0{}", "o".repeat(200)),
        "providers": {"openai": {
            "base_url": format!("https://example.test/{}", "p".repeat(2100)),
            "api_key_env": format!("ENV_{}", "X".repeat(200))
        }}
    }))
    .unwrap();
    let projected = values(&settings);
    assert_eq!(projected.model, DEFAULT_MODEL);
    assert!(projected.model_routing.default.is_empty());
    assert!(projected.model_routing.summarization.is_empty());
    assert!(projected.output_style.is_empty());
    let provider = provider(settings.providers.openai.as_ref());
    assert!(provider.base_url.is_empty());
    assert!(provider.api_key_env.is_empty());
}
