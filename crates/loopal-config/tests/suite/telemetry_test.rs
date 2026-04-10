use loopal_config::{Settings, TelemetryConfig};

// ── Default state ─────────────────────────────────────────────

#[test]
fn default_is_enabled() {
    let config = TelemetryConfig::default();
    assert!(config.enabled);
    assert!(config.is_enabled());
}

#[test]
fn enabled_from_field() {
    let config = TelemetryConfig {
        enabled: true,
        ..Default::default()
    };
    assert!(config.is_enabled());
}

// ── Traces enabled logic ──────────────────────────────────────

#[test]
fn traces_enabled_defaults_true_when_enabled() {
    let config = TelemetryConfig {
        enabled: true,
        ..Default::default()
    };
    assert!(config.traces_enabled());
}

#[test]
fn traces_disabled_when_explicit_false() {
    let config = TelemetryConfig {
        enabled: true,
        traces: Some(false),
        ..Default::default()
    };
    assert!(!config.traces_enabled());
}

#[test]
fn traces_disabled_when_telemetry_disabled() {
    let config = TelemetryConfig {
        enabled: false,
        traces: Some(true),
        ..Default::default()
    };
    assert!(!config.traces_enabled());
}

// ── Sample rate ───────────────────────────────────────────────

#[test]
fn sample_rate_default_is_one() {
    let config = TelemetryConfig::default();
    assert!((config.sample_rate() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn sample_rate_clamped_above() {
    let config = TelemetryConfig {
        sample_rate: Some(2.5),
        ..Default::default()
    };
    assert!((config.sample_rate() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn sample_rate_clamped_below() {
    let config = TelemetryConfig {
        sample_rate: Some(-0.5),
        ..Default::default()
    };
    assert!(config.sample_rate().abs() < f64::EPSILON);
}

#[test]
fn sample_rate_valid_fraction() {
    let config = TelemetryConfig {
        sample_rate: Some(0.3),
        ..Default::default()
    };
    assert!((config.sample_rate() - 0.3).abs() < f64::EPSILON);
}

// ── Serde ─────────────────────────────────────────────────────

#[test]
fn serde_roundtrip() {
    let config = TelemetryConfig {
        enabled: true,
        otlp_endpoint: Some("http://collector:4317".into()),
        traces: Some(false),
        sample_rate: Some(0.5),
        ..Default::default()
    };
    let json = serde_json::to_string(&config).unwrap();
    let back: TelemetryConfig = serde_json::from_str(&json).unwrap();
    assert!(back.enabled);
    assert_eq!(back.otlp_endpoint.as_deref(), Some("http://collector:4317"));
    assert_eq!(back.traces, Some(false));
    assert!((back.sample_rate.unwrap() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn serde_from_empty_json() {
    let config: TelemetryConfig = serde_json::from_str("{}").unwrap();
    assert!(config.enabled);
    assert!(config.otlp_endpoint.is_none());
    assert!(config.traces.is_none());
    assert!(config.metrics.is_none());
    assert!(config.logs.is_none());
    assert!(config.sample_rate.is_none());
}

#[test]
fn serde_partial_override() {
    let config: TelemetryConfig = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
    assert!(config.enabled);
    assert!(config.otlp_endpoint.is_none()); // kept default
    assert!(config.traces.is_none()); // kept default
}

#[test]
fn settings_with_telemetry_merge() {
    let json = r#"{"telemetry": {"enabled": true, "sample_rate": 0.5}}"#;
    let settings: Settings = serde_json::from_str(json).unwrap();
    assert!(settings.telemetry.enabled);
    assert!((settings.telemetry.sample_rate.unwrap() - 0.5).abs() < f64::EPSILON);
    // Other settings remain default
    assert_eq!(settings.model, "claude-sonnet-4-20250514");
}

#[test]
fn settings_without_telemetry_has_default() {
    let json = r#"{"model": "gpt-4"}"#;
    let settings: Settings = serde_json::from_str(json).unwrap();
    assert!(settings.telemetry.enabled);
}
