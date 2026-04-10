//! JSONL file exporter for OpenTelemetry metrics.

use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use async_trait::async_trait;
use opentelemetry_sdk::metrics::Temporality;
use opentelemetry_sdk::metrics::data::{Metric, ResourceMetrics};
use opentelemetry_sdk::metrics::exporter::PushMetricExporter;
use serde::Serialize;

/// Metric exporter that appends JSONL data points to a local file.
pub(crate) struct JsonlMetricExporter {
    writer: Mutex<BufWriter<File>>,
}

impl Debug for JsonlMetricExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonlMetricExporter").finish()
    }
}

impl JsonlMetricExporter {
    pub fn new(dir: &PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let pid = std::process::id();
        let path = dir.join(format!("metrics-{ts}-{pid}.jsonl"));
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
        })
    }

    fn write_metric(&self, metric: &Metric) {
        let now_ms = super::file_span_exporter::epoch_ms(std::time::SystemTime::now());
        let name = &metric.name;
        let unit = &metric.unit;
        let data = &metric.data;
        let data_any = data.as_any();

        if let Some(sum) = data_any.downcast_ref::<opentelemetry_sdk::metrics::data::Sum<f64>>() {
            for dp in &sum.data_points {
                self.write_point(now_ms, name, unit, "sum", dp.value, &dp.attributes);
            }
        } else if let Some(sum) =
            data_any.downcast_ref::<opentelemetry_sdk::metrics::data::Sum<u64>>()
        {
            for dp in &sum.data_points {
                self.write_point(now_ms, name, unit, "sum", dp.value as f64, &dp.attributes);
            }
        } else if let Some(sum) =
            data_any.downcast_ref::<opentelemetry_sdk::metrics::data::Sum<i64>>()
        {
            for dp in &sum.data_points {
                self.write_point(now_ms, name, unit, "sum", dp.value as f64, &dp.attributes);
            }
        } else if let Some(hist) =
            data_any.downcast_ref::<opentelemetry_sdk::metrics::data::Histogram<f64>>()
        {
            for dp in &hist.data_points {
                self.write_histogram(now_ms, name, unit, dp);
            }
        } else if let Some(gauge) =
            data_any.downcast_ref::<opentelemetry_sdk::metrics::data::Gauge<f64>>()
        {
            for dp in &gauge.data_points {
                self.write_point(now_ms, name, unit, "gauge", dp.value, &dp.attributes);
            }
        } else if let Some(gauge) =
            data_any.downcast_ref::<opentelemetry_sdk::metrics::data::Gauge<i64>>()
        {
            for dp in &gauge.data_points {
                self.write_point(now_ms, name, unit, "gauge", dp.value as f64, &dp.attributes);
            }
        }
    }

    fn write_point(
        &self,
        ts_ms: u64,
        name: &str,
        unit: &str,
        kind: &str,
        value: f64,
        attrs: &[opentelemetry::KeyValue],
    ) {
        let record = MetricRecord {
            ts_ms,
            name,
            unit,
            kind,
            value,
            count: None,
            sum: None,
            min: None,
            max: None,
            attributes: kv_to_strings(attrs),
        };
        self.write_json(&record);
    }

    fn write_histogram(
        &self,
        ts_ms: u64,
        name: &str,
        unit: &str,
        dp: &opentelemetry_sdk::metrics::data::HistogramDataPoint<f64>,
    ) {
        let record = MetricRecord {
            ts_ms,
            name,
            unit,
            kind: "histogram",
            value: 0.0,
            count: Some(dp.count),
            sum: Some(dp.sum),
            min: dp.min,
            max: dp.max,
            attributes: kv_to_strings(&dp.attributes),
        };
        self.write_json(&record);
    }

    fn write_json(&self, record: &MetricRecord) {
        if let Ok(json) = serde_json::to_string(record)
            && let Ok(mut w) = self.writer.lock()
        {
            let _ = writeln!(w, "{json}");
        }
    }

    fn flush(&self) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.flush();
        }
    }
}

#[async_trait]
impl PushMetricExporter for JsonlMetricExporter {
    async fn export(
        &self,
        metrics: &mut ResourceMetrics,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        for scope in &metrics.scope_metrics {
            for metric in &scope.metrics {
                self.write_metric(metric);
            }
        }
        self.flush();
        Ok(())
    }

    async fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.flush();
        Ok(())
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.flush();
        Ok(())
    }

    fn temporality(&self) -> Temporality {
        Temporality::Cumulative
    }
}

fn kv_to_strings(attrs: &[opentelemetry::KeyValue]) -> Vec<(String, String)> {
    attrs
        .iter()
        .map(|kv| (kv.key.to_string(), kv.value.to_string()))
        .collect()
}

#[derive(Serialize)]
struct MetricRecord<'a> {
    ts_ms: u64,
    name: &'a str,
    unit: &'a str,
    kind: &'a str,
    value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<f64>,
    attributes: Vec<(String, String)>,
}
