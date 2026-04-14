//! Graceful shutdown guard for OTel providers and the log writer.

use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;

/// Guard that flushes and shuts down OTel providers and the log writer on drop.
///
/// Hold this in the scope of `main()` to ensure all buffered telemetry
/// and log output is exported before process exit.
pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    logger_provider: Option<SdkLoggerProvider>,
    _log_guard: tracing_appender::non_blocking::WorkerGuard,
}

impl TelemetryGuard {
    pub fn new(
        tracer_provider: Option<SdkTracerProvider>,
        meter_provider: Option<SdkMeterProvider>,
        logger_provider: Option<SdkLoggerProvider>,
        log_guard: tracing_appender::non_blocking::WorkerGuard,
    ) -> Self {
        Self {
            tracer_provider,
            meter_provider,
            logger_provider,
            _log_guard: log_guard,
        }
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(tp) = self.tracer_provider.take()
            && let Err(e) = tp.shutdown()
        {
            tracing::warn!("otel tracer shutdown error: {e}");
        }
        if let Some(mp) = self.meter_provider.take()
            && let Err(e) = mp.shutdown()
        {
            tracing::warn!("otel meter shutdown error: {e}");
        }
        if let Some(lp) = self.logger_provider.take()
            && let Err(e) = lp.shutdown()
        {
            tracing::warn!("otel logger shutdown error: {e}");
        }
    }
}
