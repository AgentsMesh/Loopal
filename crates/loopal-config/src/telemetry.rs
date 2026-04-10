//! OpenTelemetry configuration for the telemetry subsystem.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Configuration for OpenTelemetry-based observability.
///
/// When `enabled` is false, no OTel subscriber is registered
/// and the existing file-based logging continues with zero overhead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    /// Enable OTel export (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// OTLP endpoint (default: "http://localhost:4317").
    pub otlp_endpoint: Option<String>,

    /// Enable traces export (default: true when enabled).
    pub traces: Option<bool>,

    /// Enable metrics export (default: true when enabled).
    pub metrics: Option<bool>,

    /// Enable OTel logs export (default: false — file logs already exist).
    pub logs: Option<bool>,

    /// Trace sampling ratio 0.0–1.0 (default: 1.0).
    pub sample_rate: Option<f64>,

    /// Export telemetry to local JSON Lines files (default: true when enabled).
    pub file_export: Option<bool>,

    /// Directory for JSONL telemetry files (default: ~/.loopal/telemetry/).
    pub telemetry_dir: Option<String>,
}

impl TelemetryConfig {
    /// Effective OTLP endpoint, checking env override first.
    pub fn endpoint(&self) -> String {
        std::env::var("LOOPAL_OTEL_ENDPOINT")
            .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
            .unwrap_or_else(|_| {
                self.otlp_endpoint
                    .clone()
                    .unwrap_or_else(|| "http://localhost:4317".to_string())
            })
    }

    /// Whether traces are enabled (defaults to true when telemetry is enabled).
    pub fn traces_enabled(&self) -> bool {
        self.is_enabled() && self.traces.unwrap_or(true)
    }

    /// Whether metrics are enabled (defaults to true when telemetry is enabled).
    pub fn metrics_enabled(&self) -> bool {
        self.is_enabled() && self.metrics.unwrap_or(true)
    }

    /// Whether OTel logs export is enabled (defaults to false).
    pub fn logs_enabled(&self) -> bool {
        self.is_enabled() && self.logs.unwrap_or(false)
    }

    /// Effective sample rate (0.0–1.0).
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate.unwrap_or(1.0).clamp(0.0, 1.0)
    }

    /// Whether JSONL file export is enabled (defaults to true when telemetry is enabled).
    pub fn file_export_enabled(&self) -> bool {
        self.is_enabled() && self.file_export.unwrap_or(true)
    }

    /// Effective telemetry directory for JSONL files.
    pub fn telemetry_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.telemetry_dir {
            return PathBuf::from(dir);
        }
        crate::locations::global_config_dir()
            .map(|d| d.join("telemetry"))
            .unwrap_or_else(|_| crate::locations::volatile_dir().join("telemetry"))
    }

    /// Check if telemetry is effectively enabled (config or env override).
    pub fn is_enabled(&self) -> bool {
        // Env var can force-disable even when config says enabled.
        if let Ok(v) = std::env::var("LOOPAL_OTEL_ENABLED") {
            return v == "1" || v.eq_ignore_ascii_case("true");
        }
        self.enabled
    }
}

fn default_true() -> bool {
    true
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            otlp_endpoint: None,
            traces: None,
            metrics: None,
            logs: None,
            sample_rate: None,
            file_export: None,
            telemetry_dir: None,
        }
    }
}
