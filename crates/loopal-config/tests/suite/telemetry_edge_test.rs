use loopal_config::TelemetryConfig;

// ── Metrics enabled logic ────────────────────────────────────

#[test]
fn metrics_enabled_defaults_true_when_enabled() {
    let config = TelemetryConfig {
        enabled: true,
        ..Default::default()
    };
    assert!(config.metrics_enabled());
}

#[test]
fn metrics_disabled_when_explicit_false() {
    let config = TelemetryConfig {
        enabled: true,
        metrics: Some(false),
        ..Default::default()
    };
    assert!(!config.metrics_enabled());
}

#[test]
fn metrics_disabled_when_telemetry_disabled() {
    let config = TelemetryConfig {
        enabled: false,
        metrics: Some(true),
        ..Default::default()
    };
    assert!(!config.metrics_enabled());
}

// ── Logs enabled logic ───────────────────────────────────────

#[test]
fn logs_disabled_by_default_when_enabled() {
    let config = TelemetryConfig {
        enabled: true,
        ..Default::default()
    };
    // Unlike traces/metrics, logs defaults to false
    assert!(!config.logs_enabled());
}

#[test]
fn logs_enabled_when_explicit_true() {
    let config = TelemetryConfig {
        enabled: true,
        logs: Some(true),
        ..Default::default()
    };
    assert!(config.logs_enabled());
}

#[test]
fn logs_disabled_when_telemetry_disabled() {
    let config = TelemetryConfig {
        enabled: false,
        logs: Some(true),
        ..Default::default()
    };
    assert!(!config.logs_enabled());
}
