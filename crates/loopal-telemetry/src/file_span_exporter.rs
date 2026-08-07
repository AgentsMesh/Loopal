//! JSONL file exporter for OpenTelemetry spans.
//!
//! Writes one JSON object per line for each completed span.
//! Designed for offline analysis with `jq`, `pandas`, etc.

use std::fmt::Debug;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::SystemTime;

use opentelemetry::trace::{SpanKind, Status};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{SpanData, SpanExporter};
use serde::Serialize;

use crate::lazy_jsonl::LazyJsonlFile;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Span exporter that appends one JSONL line per span to a local file.
pub(crate) struct JsonlSpanExporter {
    sink: LazyJsonlFile,
}

impl Debug for JsonlSpanExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonlSpanExporter").finish()
    }
}

impl JsonlSpanExporter {
    pub fn new(dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let pid = std::process::id();
        let path = dir.join(format!("traces-{ts}-{pid}.jsonl"));
        Ok(Self {
            sink: LazyJsonlFile::new(path),
        })
    }

    fn write_span(&mut self, span: &SpanData) {
        let record = SpanRecord {
            ts_ms: epoch_ms(span.start_time),
            trace_id: span.span_context.trace_id().to_string(),
            span_id: span.span_context.span_id().to_string(),
            parent_span_id: span.parent_span_id.to_string(),
            name: span.name.to_string(),
            kind: span_kind_name(&span.span_kind),
            duration_ms: span
                .end_time
                .duration_since(span.start_time)
                .unwrap_or_default()
                .as_millis() as u64,
            status: match &span.status {
                Status::Unset => "unset",
                Status::Ok => "ok",
                Status::Error { .. } => "error",
            },
            attributes: span
                .attributes
                .iter()
                .map(|kv| (kv.key.to_string(), kv.value.to_string()))
                .collect(),
        };
        if let Ok(json) = serde_json::to_string(&record) {
            self.sink.write_line(&json);
        }
    }
}

impl SpanExporter for JsonlSpanExporter {
    fn export(
        &mut self,
        batch: Vec<SpanData>,
    ) -> BoxFuture<opentelemetry_sdk::error::OTelSdkResult> {
        for span in &batch {
            self.write_span(span);
        }
        self.sink.flush();
        Box::pin(std::future::ready(Ok(())))
    }

    fn shutdown(&mut self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.sink.flush();
        Ok(())
    }

    fn set_resource(&mut self, _resource: &Resource) {}
}

#[derive(Serialize)]
struct SpanRecord<'a> {
    ts_ms: u64,
    trace_id: String,
    span_id: String,
    parent_span_id: String,
    name: String,
    kind: &'a str,
    duration_ms: u64,
    status: &'a str,
    attributes: Vec<(String, String)>,
}

fn span_kind_name(kind: &SpanKind) -> &'static str {
    match kind {
        SpanKind::Client => "client",
        SpanKind::Server => "server",
        SpanKind::Producer => "producer",
        SpanKind::Consumer => "consumer",
        SpanKind::Internal => "internal",
    }
}

pub(crate) fn epoch_ms(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use opentelemetry::trace::SpanKind;

    use super::span_kind_name;

    #[test]
    fn span_kinds_have_stable_json_names() {
        assert_eq!(span_kind_name(&SpanKind::Client), "client");
        assert_eq!(span_kind_name(&SpanKind::Server), "server");
        assert_eq!(span_kind_name(&SpanKind::Producer), "producer");
        assert_eq!(span_kind_name(&SpanKind::Consumer), "consumer");
        assert_eq!(span_kind_name(&SpanKind::Internal), "internal");
    }
}
