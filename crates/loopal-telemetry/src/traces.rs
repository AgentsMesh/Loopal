//! TracerProvider setup with OTLP + optional JSONL file exporter.

use loopal_config::TelemetryConfig;
use opentelemetry::trace::TraceError;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};

use crate::resource::build_resource;

/// Build a TracerProvider with OTLP and/or JSONL file exporter.
pub(crate) fn build_tracer_provider(
    config: &TelemetryConfig,
) -> Result<SdkTracerProvider, TraceError> {
    let sampler = if (config.sample_rate() - 1.0).abs() < f64::EPSILON {
        Sampler::AlwaysOn
    } else if config.sample_rate() < f64::EPSILON {
        Sampler::AlwaysOff
    } else {
        Sampler::TraceIdRatioBased(config.sample_rate())
    };

    let mut builder = SdkTracerProvider::builder()
        .with_sampler(sampler)
        .with_resource(build_resource());

    // OTLP exporter (sends to remote collector).
    let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(config.endpoint())
        .build()?;
    builder = builder.with_batch_exporter(otlp_exporter);

    // JSONL file exporter (local analysis).
    if config.file_export_enabled() {
        match crate::file_span_exporter::JsonlSpanExporter::new(&config.telemetry_dir()) {
            Ok(exporter) => builder = builder.with_batch_exporter(exporter),
            Err(e) => eprintln!("otel: failed to create JSONL span exporter: {e}"),
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
    async fn build_provider_succeeds_with_defaults() {
        let result = build_tracer_provider(&enabled_config());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn build_provider_respects_sample_rate() {
        let config = TelemetryConfig {
            enabled: true,
            sample_rate: Some(0.0),
            ..Default::default()
        };
        let result = build_tracer_provider(&config);
        assert!(result.is_ok());
    }
}
