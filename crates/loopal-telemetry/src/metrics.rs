//! OTel MeterProvider setup with OTLP + optional JSONL file exporter.

use loopal_config::TelemetryConfig;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::metrics::SdkMeterProvider;

use crate::resource::build_resource;

/// Build a MeterProvider with OTLP and/or JSONL file periodic reader.
pub(crate) fn build_meter_provider(
    config: &TelemetryConfig,
    warnings: &mut Vec<String>,
) -> Result<SdkMeterProvider, opentelemetry_sdk::metrics::MetricError> {
    let interval = std::time::Duration::from_secs(60);

    let mut builder = SdkMeterProvider::builder().with_resource(build_resource());

    // OTLP periodic reader (sends to remote collector).
    let otlp_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(config.endpoint())
        .build()?;
    let otlp_reader = opentelemetry_sdk::metrics::PeriodicReader::builder(otlp_exporter)
        .with_interval(interval)
        .build();
    builder = builder.with_reader(otlp_reader);

    // JSONL file periodic reader (local analysis).
    if config.file_export_enabled() {
        match crate::file_metric_exporter::JsonlMetricExporter::new(&config.telemetry_dir()) {
            Ok(exporter) => {
                let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
                    .with_interval(interval)
                    .build();
                builder = builder.with_reader(reader);
            }
            Err(e) => warnings.push(format!("otel: failed to create JSONL metric exporter: {e}")),
        }
    }

    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_config() -> TelemetryConfig {
        TelemetryConfig {
            enabled: true,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn build_meter_provider_succeeds() {
        let result = build_meter_provider(&enabled_config(), &mut Vec::new());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn jsonl_exporter_failure_pushes_warning() {
        let bad_dir = if cfg!(windows) {
            "Z:\\__no_such_drive__\\otel".to_string()
        } else {
            "/nonexistent/otel-dir".to_string()
        };
        let config = TelemetryConfig {
            enabled: true,
            file_export: Some(true),
            telemetry_dir: Some(bad_dir),
            ..Default::default()
        };
        let mut warnings = Vec::new();
        let _ = build_meter_provider(&config, &mut warnings);
        assert!(
            warnings.iter().any(|w| w.contains("JSONL metric exporter")),
            "expected JSONL exporter warning, got: {warnings:?}"
        );
    }
}
