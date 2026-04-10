//! OpenTelemetry observability integration for Loopal.
//!
//! Provides a layered `tracing` subscriber that bridges existing
//! `tracing` spans and events to OpenTelemetry traces via OTLP.
//! When telemetry is disabled (default), only the file-based fmt layer
//! is active with zero OTel overhead.

pub(crate) mod file_metric_exporter;
pub(crate) mod file_span_exporter;
mod logs;
mod metrics;
mod resource;
mod shutdown;
mod subscriber;
mod traces;

pub use shutdown::TelemetryGuard;
pub use subscriber::init_subscriber;
