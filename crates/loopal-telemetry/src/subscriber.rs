//! Layered tracing subscriber construction.
//!
//! Builds a `tracing_subscriber::Registry` with:
//! - **fmt layer** — file-based logging (always active)
//! - **OTel layer** — tracing-opentelemetry bridge (when enabled)

use loopal_config::TelemetryConfig;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

use crate::shutdown::TelemetryGuard;

/// Initialize the global tracing subscriber with optional OTel layer.
///
/// Returns a guard that must be held until process exit — it keeps the
/// non-blocking log writer alive and flushes OTel pipelines on drop.
pub fn init_subscriber(
    config: &TelemetryConfig,
    writer: impl std::io::Write + Send + 'static,
    env_filter: EnvFilter,
) -> TelemetryGuard {
    let (non_blocking, log_guard) = tracing_appender::non_blocking(writer);
    let mut init_warnings: Vec<String> = Vec::new();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE);

    let tracer_provider = if config.traces_enabled() {
        match crate::traces::build_tracer_provider(config, &mut init_warnings) {
            Ok(tp) => Some(tp),
            Err(e) => {
                init_warnings.push(format!("otel: failed to build tracer provider: {e}"));
                None
            }
        }
    } else {
        None
    };

    let otel_layer = tracer_provider.as_ref().map(|tp| {
        let tracer = opentelemetry::trace::TracerProvider::tracer(tp, "loopal");
        tracing_opentelemetry::layer().with_tracer(tracer).boxed()
    });

    let meter_provider = if config.metrics_enabled() {
        match crate::metrics::build_meter_provider(config, &mut init_warnings) {
            Ok(mp) => {
                opentelemetry::global::set_meter_provider(mp.clone());
                Some(mp)
            }
            Err(e) => {
                init_warnings.push(format!("otel: failed to build meter provider: {e}"));
                None
            }
        }
    } else {
        None
    };

    let logger_provider = if config.logs_enabled() {
        match crate::logs::build_logger_provider(config) {
            Ok(lp) => Some(lp),
            Err(e) => {
                init_warnings.push(format!("otel: failed to build logger provider: {e}"));
                None
            }
        }
    } else {
        None
    };

    let logs_layer = logger_provider.as_ref().map(|lp| {
        opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(lp).boxed()
    });

    Registry::default()
        .with(otel_layer)
        .with(logs_layer)
        .with(env_filter)
        .with(fmt_layer)
        .init();

    for msg in &init_warnings {
        tracing::warn!("{msg}");
    }

    TelemetryGuard::new(tracer_provider, meter_provider, logger_provider, log_guard)
}
