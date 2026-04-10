//! OTel LoggerProvider setup with OTLP exporter.

use loopal_config::TelemetryConfig;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;

use crate::resource::build_resource;

/// Build a LoggerProvider configured with an OTLP exporter.
pub(crate) fn build_logger_provider(
    config: &TelemetryConfig,
) -> Result<SdkLoggerProvider, opentelemetry_sdk::logs::LogError> {
    let exporter = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .with_endpoint(config.endpoint())
        .build()?;

    let provider = SdkLoggerProvider::builder()
        .with_resource(build_resource())
        .with_batch_exporter(exporter)
        .build();

    Ok(provider)
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
    async fn build_logger_provider_succeeds() {
        let result = build_logger_provider(&enabled_config());
        assert!(result.is_ok());
    }
}
